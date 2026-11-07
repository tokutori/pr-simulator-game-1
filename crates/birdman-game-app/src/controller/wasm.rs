//! Bounded deterministic guest execution; no platform imports or independent game owner.

use birdman_game_format::external_controller::{
    ABI_VERSION, ControllerFault, ControllerFrame, ExchangeKind, MAX_FRAME_BYTES, MAX_MODULE_BYTES,
};
use wasmi::{
    CompilationMode, Config, EnforcedLimits, Engine, Linker, Memory, Module, ResourceLimiter,
    Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

#[derive(Clone)]
pub(super) struct WasmArtifact {
    engine: Engine,
    module: Module,
}

fn invalid(cause: impl core::fmt::Display) -> String {
    format!("Controller Wasm: {cause}")
}

impl WasmArtifact {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        preflight(bytes)?;
        let mut config = Config::default();
        config
            .consume_fuel(true)
            .allow_start_fn(false)
            .compilation_mode(CompilationMode::Eager)
            .enforced_limits(EnforcedLimits::strict())
            .set_min_stack_height(256 * 1024)
            .set_max_stack_height(256 * 1024)
            .set_max_recursion_depth(128)
            .set_max_cached_stacks(1)
            .wasm_multi_memory(false)
            .wasm_reference_types(true)
            .wasm_tail_call(false);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes).map_err(invalid)?;
        if module.imports().next().is_some() {
            return Err("Controller Wasm imports must be empty".into());
        }
        Ok(Self { engine, module })
    }
    pub(super) fn instantiate(&self, init: &ControllerFrame) -> Result<WasmExecution, String> {
        let limits = StoreLimitsBuilder::new()
            .memory_size(2 * 1024 * 1024)
            .table_elements(4096)
            .instances(1)
            .memories(1)
            .tables(1)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            &self.engine,
            GuestLimits {
                limits,
                sealed: false,
                denied: false,
            },
        );
        store.limiter(|state| state);
        store.set_fuel(10_000).map_err(invalid)?;
        let instance = Linker::<GuestLimits>::new(&self.engine)
            .instantiate_and_start(&mut store, &self.module)
            .map_err(invalid)?;
        let version = instance
            .get_typed_func::<(), i32>(&store, "controller_abi_version")
            .map_err(invalid)?;
        store.set_fuel(10_000).map_err(invalid)?;
        if version.call(&mut store, ()).map_err(invalid)? != i32::from(ABI_VERSION) {
            return Err("Controller ABI version is unsupported".into());
        }
        let buffer = instance
            .get_typed_func::<(), i32>(&store, "controller_buffer")
            .map_err(invalid)?;
        store.set_fuel(10_000).map_err(invalid)?;
        let offset =
            usize::try_from(buffer.call(&mut store, ()).map_err(invalid)?).map_err(invalid)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| String::from("Controller Wasm memory export is missing"))?;
        if offset
            .checked_add(2 * MAX_FRAME_BYTES)
            .is_none_or(|end| end > memory.data_size(&store))
        {
            return Err("Controller ABI buffer is outside its memory".into());
        }
        let exchange = instance
            .get_typed_func::<i32, i32>(&store, "controller_exchange")
            .map_err(invalid)?;
        let mut execution = WasmExecution {
            store,
            memory,
            exchange,
            offset,
        };
        execution
            .exchange(init)
            .map_err(|cause| format!("Controller initialization failed: {cause:?}"))?;
        execution.store.data_mut().sealed = true;
        Ok(execution)
    }
}

pub(super) struct WasmExecution {
    store: Store<GuestLimits>,
    memory: Memory,
    exchange: TypedFunc<i32, i32>,
    offset: usize,
}
impl WasmExecution {
    pub(super) fn exchange(
        &mut self,
        frame: &ControllerFrame,
    ) -> Result<ControllerFrame, ControllerFault> {
        let fuel = match frame.header().kind {
            ExchangeKind::Init => 2_000_000,
            ExchangeKind::Evaluate => 200_000,
            ExchangeKind::Accepted => 100_000,
            ExchangeKind::Finish => 50_000,
        };
        self.store
            .set_fuel(fuel)
            .map_err(|_| ControllerFault::ResourceLimit)?;
        self.memory
            .write(&mut self.store, self.offset, frame.as_bytes())
            .map_err(|_| ControllerFault::BadFrame)?;
        self.store.data_mut().denied = false;
        let result = self
            .exchange
            .call(&mut self.store, frame.as_bytes().len() as i32);
        let length = result.map_err(|cause| {
            if self.store.data().denied {
                return ControllerFault::ResourceLimit;
            }
            match cause.as_trap_code() {
                Some(wasmi::TrapCode::OutOfFuel) => ControllerFault::FuelExhausted,
                Some(wasmi::TrapCode::StackOverflow) => ControllerFault::ResourceLimit,
                _ => ControllerFault::Trap,
            }
        })?;
        let length = usize::try_from(length).map_err(|_| ControllerFault::BadFrame)?;
        if length > MAX_FRAME_BYTES {
            return Err(ControllerFault::BadFrame);
        }
        let mut bytes = [0; MAX_FRAME_BYTES];
        self.memory
            .read(
                &self.store,
                self.offset + MAX_FRAME_BYTES,
                &mut bytes[..length],
            )
            .map_err(|_| ControllerFault::BadFrame)?;
        let reply =
            ControllerFrame::decode_reply(&bytes[..length], frame).map_err(
                |cause| match cause {
                    birdman_game_format::external_controller::ProtocolError::NonFiniteValue => {
                        ControllerFault::NonFiniteOutput
                    }
                    birdman_game_format::external_controller::ProtocolError::PeerRejected => {
                        ControllerFault::GuestRejected
                    }
                    _ => ControllerFault::BadFrame,
                },
            )?;
        Ok(reply)
    }
}

fn preflight(bytes: &[u8]) -> Result<(), String> {
    use wasmparser::{Parser, Payload};
    if bytes.len() > MAX_MODULE_BYTES {
        return Err("Controller Wasm exceeds 1 MiB".into());
    }
    let mut locals_total = 0u32;
    for payload in Parser::new(0).parse_all(bytes) {
        let admitted = match payload.map_err(invalid)? {
            Payload::Version { num, encoding, .. } => {
                num == 1 && encoding == wasmparser::Encoding::Module
            }
            Payload::ImportSection(reader) => reader.count() == 0,
            Payload::TypeSection(reader) => reader.count() <= 1024,
            Payload::FunctionSection(reader) => reader.count() <= 4096,
            Payload::GlobalSection(reader) => reader.count() <= 256,
            Payload::ExportSection(reader) => reader.count() <= 16,
            Payload::DataSection(reader) => reader.count() <= 128,
            Payload::ElementSection(reader) => reader.count() <= 128,
            Payload::MemorySection(reader) => {
                if reader.count() != 1 {
                    return Err("Controller Wasm requires one memory".into());
                }
                for memory in reader {
                    let m = memory.map_err(invalid)?;
                    if m.memory64 || m.shared || m.initial > 32 {
                        return Err("Controller Wasm memory is outside its profile".into());
                    }
                }
                true
            }
            Payload::TableSection(reader) => reader.count() <= 1,
            Payload::CodeSectionEntry(body) => {
                if body.range().len() > 256 * 1024 {
                    return Err("Controller function exceeds its body budget".into());
                }
                let mut count = 0u32;
                for entry in body.get_locals_reader().map_err(invalid)? {
                    count = count
                        .checked_add(entry.map_err(invalid)?.0)
                        .ok_or_else(|| String::from("Controller local count overflow"))?;
                }
                locals_total = locals_total
                    .checked_add(count)
                    .ok_or_else(|| String::from("Controller local count overflow"))?;
                count <= 8192 && locals_total <= 65536
            }
            Payload::StartSection { .. } => false,
            _ => true,
        };
        if !admitted {
            return Err("Controller Wasm exceeds its fixed module profile".into());
        }
    }
    Ok(())
}

struct GuestLimits {
    limits: StoreLimits,
    sealed: bool,
    denied: bool,
}
impl ResourceLimiter for GuestLimits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool, wasmi_core::LimiterError> {
        if self.sealed && desired > current {
            self.denied = true;
            return Err(wasmi_core::LimiterError::ResourceLimiterDeniedAllocation);
        }
        let result = self.limits.memory_growing(current, desired, maximum);
        self.denied |= result.is_err() || matches!(result, Ok(false));
        result
    }
    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool, wasmi_core::LimiterError> {
        if self.sealed && desired > current {
            self.denied = true;
            return Err(wasmi_core::LimiterError::ResourceLimiterDeniedAllocation);
        }
        let result = self.limits.table_growing(current, desired, maximum);
        self.denied |= result.is_err() || matches!(result, Ok(false));
        result
    }
    fn instances(&self) -> usize {
        1
    }
    fn memories(&self) -> usize {
        1
    }
    fn tables(&self) -> usize {
        1
    }
}
