use birdman_game_format::external_controller::{
    AcceptedPayload, Admission, ControllerFault, ControllerFrame, EvaluatePayload, ExchangeKind,
    FinishPayload, FinishReason, FrameHeader, HEADER_BYTES, InitPayload, PeerStatus,
    REQUIRED_OBSERVATIONS,
};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn one_process_exchanges_init_candidate_admission_and_finish_then_retires_at_eof() {
    let header = FrameHeader {
        kind: ExchangeKind::Init,
        status: PeerStatus::Success,
        generation: 7,
        sequence: 0,
        identity: [9; 32],
    };
    let mut init = InitPayload {
        pitch_template: 0,
        yaw_template: 0,
        requested_mode: 0,
        flags: 0,
        values: [0.0; 71],
    };
    for index in [3, 4] {
        init.values[index * 8..index * 8 + 8]
            .copy_from_slice(&[0.0, 0.0, 0.0, 0.0, 0.0, -0.2, 0.2, 0.0]);
    }
    init.values[46..54].copy_from_slice(&[-0.2, 0.2, -0.2, 0.2, -0.2, 0.2, -0.2, 0.2]);
    init.values[55] = 10.0;
    init.values[58] = 0.5;
    init.values[60] = 100.0;
    init.values[65..71].copy_from_slice(&[-0.3, 0.3, -0.3, 0.3, 1.0, 1.0]);
    let mut values = [0.0; 40];
    values[1] = 0.01;
    values[3] = 1.0;
    values[22] = 10.0;
    let requests = [
        ControllerFrame::init(header, init).unwrap(),
        ControllerFrame::evaluate(
            FrameHeader {
                kind: ExchangeKind::Evaluate,
                sequence: 1,
                ..header
            },
            EvaluatePayload {
                tick: 0,
                valid_mask: REQUIRED_OBSERVATIONS,
                pilot_command_kind: 0,
                values,
            },
        )
        .unwrap(),
        ControllerFrame::accepted(
            FrameHeader {
                kind: ExchangeKind::Accepted,
                sequence: 1,
                ..header
            },
            AcceptedPayload {
                tick: 0,
                admission: Admission::Full,
                obstruction_mask: 0,
                values: [0.01, 0.01, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
        )
        .unwrap(),
        ControllerFrame::finish(
            FrameHeader {
                kind: ExchangeKind::Finish,
                sequence: 2,
                ..header
            },
            FinishPayload {
                tick: 1,
                reason: FinishReason::TimeLimit,
                fault: ControllerFault::None,
                fault_tick: 0,
                accepted_sequence: 1,
                values: [0.01, 0.01, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
        )
        .unwrap(),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_controller-sample-stdio"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        // Four successful replies total 272 bytes, so output fits while input is streamed.
        for request in &requests {
            input.write_all(request.as_bytes()).unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let mut bytes = output.stdout.as_slice();
    for request in &requests {
        assert!(bytes.len() >= HEADER_BYTES);
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let reply = ControllerFrame::decode_reply(&bytes[..length], request).unwrap();
        if request.header().kind == ExchangeKind::Evaluate {
            assert_eq!(reply.requested_angles().unwrap(), [0.0; 2]);
        }
        bytes = &bytes[length..];
    }
    assert!(bytes.is_empty());
}
