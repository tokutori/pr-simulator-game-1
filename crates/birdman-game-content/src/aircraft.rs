use alloc::vec::Vec;
use birdman_game_core::{
    AeroError, AircraftModel, BodyPoint, BodyVector, ElementOrientation, ElementReference,
    HybridAnchor, HybridError, HybridModel, HybridProxy, HybridSection, HybridSurface,
    HybridSurfaceGeometry, HybridSurfaceRole, MathError, PlanformSymmetry, PolarAnalysisMethod,
    PolarMomentAxes, StaticPolar, StaticPolarMetadata, StaticPolarRow, SteadyGlideError,
    SteadyGlideTrim, TailIncidence, WingBendingFactor,
};
use birdman_game_format::aircraft::{
    AircraftDocument, AircraftFormatError, AircraftPolarAxes, AircraftPolarMethod,
    AircraftSurfaceDocument,
};

/// Complete source of the registered fictional glider, in body FRD coordinates and SI units.
pub const PLAYABLE_AIRCRAFT_JSON: &[u8] =
    include_bytes!("../../../assets/aircraft/playable-glider-v4.json");

/// Decodes and prepares the bundled aircraft into owned immutable storage.
pub fn playable_aircraft() -> Result<AircraftDefinition, AircraftDefinitionError> {
    AircraftDefinition::decode_json(PLAYABLE_AIRCRAFT_JSON)
}

/// Original admission or preparation failure for an aircraft definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AircraftDefinitionError {
    /// External document or physical parameter validation failed.
    Format(AircraftFormatError),
    /// A derived coordinate or frame was invalid.
    Math(MathError),
    /// Static aerodynamic metadata or a local frame was invalid.
    Aerodynamics(AeroError),
    /// A derived strip partition or complete hybrid model was invalid.
    Hybrid(HybridError),
    /// A structural outer point or its sampling frame was invalid.
    Shape(crate::ShapeError),
    /// The requested operating point could not form a steady-glide equilibrium.
    Trim(SteadyGlideError),
}

/// Owned physical parameters and aerodynamic storage derived from one validated document.
pub struct AircraftDefinition {
    document: AircraftDocument,
    aircraft: AircraftModel,
    surfaces: Vec<SurfaceStorage>,
    rows: Vec<StaticPolarRow>,
    reference: ElementReference,
    contact: crate::shape::ContactSamples,
}

struct SurfaceStorage {
    role: HybridSurfaceRole,
    symmetry: PlanformSymmetry,
    sections: Vec<HybridSection>,
    proxies: Vec<HybridProxy>,
}

impl AircraftDefinition {
    /// Decodes bounded external JSON and prepares its complete physical load definition.
    pub fn decode_json(input: &[u8]) -> Result<Self, AircraftDefinitionError> {
        Self::prepare_validated(
            AircraftDocument::decode_json(input).map_err(AircraftDefinitionError::Format)?,
        )
    }

    /// Takes ownership of the source and prepares every lifting surface and static row.
    pub fn from_document(document: AircraftDocument) -> Result<Self, AircraftDefinitionError> {
        document
            .validate()
            .map_err(AircraftDefinitionError::Format)?;
        Self::prepare_validated(document)
    }

    fn prepare_validated(document: AircraftDocument) -> Result<Self, AircraftDefinitionError> {
        let aircraft = document
            .aircraft_model()
            .map_err(AircraftDefinitionError::Format)?;
        let surfaces = document
            .surfaces
            .iter()
            .map(prepare_surface)
            .collect::<Result<Vec<_>, _>>()?;
        let rows = document
            .polar
            .core_rows()
            .map_err(AircraftDefinitionError::Format)?;
        let wing = surfaces
            .iter()
            .find(|surface| surface.role == HybridSurfaceRole::MainWing)
            .expect("document validation requires exactly one main wing")
            .geometry()?;
        let reference = ElementReference::try_new(
            wing.projected_area_m2(),
            wing.projected_span_m(),
            wing.projected_mac_m(),
        )
        .map_err(AircraftDefinitionError::Aerodynamics)?;
        let contact = crate::AircraftShape::new(&document)
            .contact_samples(crate::ContactSampling::default())
            .map_err(AircraftDefinitionError::Shape)?;
        let definition = Self {
            document,
            aircraft,
            surfaces,
            rows,
            reference,
            contact,
        };
        for tip in definition
            .shape()
            .surface(HybridSurfaceRole::MainWing)
            .span_bounds_m()
        {
            definition
                .shape()
                .wing_displacement(tip, f64::MAX)
                .map_err(AircraftDefinitionError::Shape)?;
        }
        HybridModel::try_new(definition.polar()?, &definition.surfaces()?)
            .map_err(AircraftDefinitionError::Hybrid)?;
        definition.trim()?;
        Ok(definition)
    }

    /// Borrows the validated source of the physical, structural and camera parameters.
    pub fn document(&self) -> &AircraftDocument {
        &self.document
    }

    /// Borrows the physical outer shape from the same validated aircraft document.
    pub fn shape(&self) -> crate::AircraftShape<'_> {
        crate::AircraftShape::new(&self.document)
    }

    /// Borrows the prepared neutral structural polytope used by physical contact generation.
    pub fn contact_points(&self) -> &[BodyPoint] {
        &self.contact.points
    }

    /// Writes FRD support points with stable neutral identity into caller-owned storage.
    ///
    /// The caller publishes the output after success; arithmetic failure may leave a prefix written.
    pub fn write_deformed_contact_points(
        &self,
        airspeed_mps: f64,
        incidence: TailIncidence,
        output: &mut [BodyPoint],
    ) -> Result<(), crate::ShapeError> {
        let factor = self.shape().wing_bending_factor(airspeed_mps)?;
        self.write_deformed_contact_points_with_factor(factor, incidence, output)
    }

    /// Writes corresponding points from an accepted coefficient and held tail incidence.
    ///
    /// The caller publishes the output after success; arithmetic failure may leave a prefix written.
    pub fn write_deformed_contact_points_with_factor(
        &self,
        factor: WingBendingFactor,
        incidence: TailIncidence,
        output: &mut [BodyPoint],
    ) -> Result<(), crate::ShapeError> {
        self.contact
            .write_with_factor(self.shape(), factor, incidence, output)
    }

    /// Returns the mass and pilot model prepared from the same source.
    pub const fn aircraft(&self) -> AircraftModel {
        self.aircraft
    }

    /// Borrows the complete-aircraft static table with geometry-derived reference dimensions.
    pub fn polar(&self) -> Result<StaticPolar<'_>, AircraftDefinitionError> {
        let source = &self.document.polar;
        StaticPolar::try_new(
            &self.rows,
            self.reference,
            BodyPoint::try_new(
                source.reference_point_frd_m[0],
                source.reference_point_frd_m[1],
                source.reference_point_frd_m[2],
            )
            .map_err(AircraftDefinitionError::Math)?,
            match source.moment_axes {
                AircraftPolarAxes::BodyFrd => PolarMomentAxes::BodyFrd,
                AircraftPolarAxes::WindAtBetaZero => PolarMomentAxes::WindAtBetaZero,
            },
            StaticPolarMetadata::try_new(
                match source.method {
                    AircraftPolarMethod::VortexLattice => PolarAnalysisMethod::VortexLattice,
                    AircraftPolarMethod::LiftingLine => PolarAnalysisMethod::LiftingLine,
                    AircraftPolarMethod::Panel => PolarAnalysisMethod::Panel,
                    AircraftPolarMethod::SoftwareFixture => PolarAnalysisMethod::SoftwareFixture,
                },
                &self.document.configuration_id,
                self.document.model_version,
            )
            .map_err(AircraftDefinitionError::Aerodynamics)?,
        )
        .map_err(AircraftDefinitionError::Aerodynamics)
    }

    /// Borrows the prepared wing, horizontal tail and fin in canonical order.
    pub fn surfaces(&self) -> Result<[HybridSurface<'_>; 3], AircraftDefinitionError> {
        let surface = |role| {
            let storage = self
                .surfaces
                .iter()
                .find(|surface| surface.role == role)
                .expect("document validation requires every unique surface role");
            HybridSurface::try_new(storage.geometry()?, &storage.proxies)
                .map_err(AircraftDefinitionError::Hybrid)
        };
        Ok([
            surface(HybridSurfaceRole::MainWing)?,
            surface(HybridSurfaceRole::HorizontalTail)?,
            surface(HybridSurfaceRole::VerticalTail)?,
        ])
    }

    /// Solves the document's explicit still-air operating point through the generic core.
    pub fn trim(&self) -> Result<SteadyGlideTrim, AircraftDefinitionError> {
        SteadyGlideTrim::try_new(
            self.aircraft,
            self.polar()?,
            self.document
                .trim
                .core_conditions()
                .map_err(AircraftDefinitionError::Format)?,
        )
        .map_err(AircraftDefinitionError::Trim)
    }
}

impl SurfaceStorage {
    fn geometry(&self) -> Result<HybridSurfaceGeometry<'_>, AircraftDefinitionError> {
        HybridSurfaceGeometry::try_new(self.role, &self.sections, self.symmetry)
            .map_err(AircraftDefinitionError::Hybrid)
    }
}

/// Registered geometry and retained caller storage for candidate contact shapes.
pub struct AircraftContactShape<'definition, 'storage> {
    definition: &'definition AircraftDefinition,
    points: ContactPointStorage<'storage>,
    projected: Option<(WingBendingFactor, TailIncidence)>,
}

enum ContactPointStorage<'storage> {
    Borrowed(&'storage mut [BodyPoint]),
    Owned(Vec<BodyPoint>),
}

impl ContactPointStorage<'_> {
    fn points(&self) -> &[BodyPoint] {
        match self {
            Self::Borrowed(points) => points,
            Self::Owned(points) => points,
        }
    }

    fn points_mut(&mut self) -> &mut [BodyPoint] {
        match self {
            Self::Borrowed(points) => points,
            Self::Owned(points) => points,
        }
    }
}

impl<'definition, 'storage> AircraftContactShape<'definition, 'storage> {
    /// Binds storage with exactly the prepared number of corresponding structural points.
    pub fn try_new(
        definition: &'definition AircraftDefinition,
        points: &'storage mut [BodyPoint],
    ) -> Result<Self, crate::ShapeError> {
        if points.len() != definition.contact_points().len() {
            return Err(crate::ShapeError::MismatchedContactPointCount);
        }
        Ok(Self {
            definition,
            points: ContactPointStorage::Borrowed(points),
            projected: None,
        })
    }

    /// Retains a prepared allocation and its last successful structural projection.
    pub fn try_from_owned(
        definition: &'definition AircraftDefinition,
        points: Vec<BodyPoint>,
    ) -> Result<Self, crate::ShapeError> {
        if points.len() != definition.contact_points().len() {
            return Err(crate::ShapeError::MismatchedContactPointCount);
        }
        Ok(Self {
            definition,
            points: ContactPointStorage::Owned(points),
            projected: None,
        })
    }
}

impl birdman_game_core::TailContactShapeProvider for AircraftContactShape<'_, '_> {
    fn wing_bending_factor(
        &self,
        airspeed_mps: f64,
    ) -> Result<WingBendingFactor, birdman_game_core::ContactError> {
        self.definition
            .shape()
            .wing_bending_factor(airspeed_mps)
            .map_err(contact_shape_error)
    }

    fn points(
        &mut self,
        factor: WingBendingFactor,
        incidence: TailIncidence,
    ) -> Result<&[BodyPoint], birdman_game_core::ContactError> {
        if self.projected != Some((factor, incidence)) {
            self.projected = None;
            self.definition
                .write_deformed_contact_points_with_factor(
                    factor,
                    incidence,
                    self.points.points_mut(),
                )
                .map_err(contact_shape_error)?;
            self.projected = Some((factor, incidence));
        }
        Ok(self.points.points())
    }
}

fn contact_shape_error(error: crate::ShapeError) -> birdman_game_core::ContactError {
    use birdman_game_core::ContactError;
    match error {
        crate::ShapeError::Math(cause) => ContactError::Math(cause),
        crate::ShapeError::NonFinite => ContactError::Math(MathError::NonFinite),
        crate::ShapeError::MismatchedContactPointCount => ContactError::MismatchedGeometry,
        crate::ShapeError::InvalidAirspeed
        | crate::ShapeError::InvalidChordFraction
        | crate::ShapeError::OutsideSurface(_)
        | crate::ShapeError::OutsideCockpit
        | crate::ShapeError::InvalidSampling => ContactError::ShapeProjection,
    }
}

fn prepare_surface(
    source: &AircraftSurfaceDocument,
) -> Result<SurfaceStorage, AircraftDefinitionError> {
    let mut storage = SurfaceStorage {
        role: source.role.core_role(),
        symmetry: if source.mirror_span {
            PlanformSymmetry::MirrorSpan
        } else {
            PlanformSymmetry::Unrestricted
        },
        sections: source
            .core_sections()
            .map_err(AircraftDefinitionError::Format)?,
        proxies: Vec::new(),
    };
    let geometry = storage.geometry()?;
    let axis = if storage.role == HybridSurfaceRole::VerticalTail {
        2
    } else {
        1
    };
    let first_span = storage.sections[0].quarter_chord().components()[axis];
    let last_span = storage.sections[storage.sections.len() - 1]
        .quarter_chord()
        .components()[axis];
    let span = last_span - first_span;
    let anchor = HybridAnchor::try_new(source.lift_anchor[0], source.lift_anchor[1])
        .map_err(AircraftDefinitionError::Aerodynamics)?;
    let mut proxies = Vec::new();
    for pair in storage.sections.windows(2) {
        let first = pair[0].quarter_chord().components();
        let second = pair[1].quarter_chord().components();
        let direction_y = second[1] - first[1];
        let direction_z = second[2] - first[2];
        let length = libm::hypot(direction_y, direction_z);
        let orientation = ElementOrientation::try_new(
            BodyVector::try_new(1.0, 0.0, 0.0).map_err(AircraftDefinitionError::Math)?,
            BodyVector::try_new(0.0, direction_y / length, direction_z / length)
                .map_err(AircraftDefinitionError::Math)?,
            BodyVector::try_new(0.0, -direction_z / length, direction_y / length)
                .map_err(AircraftDefinitionError::Math)?,
        )
        .map_err(AircraftDefinitionError::Aerodynamics)?;
        let mut start = first[axis];
        for end in source
            .strip_fractions
            .iter()
            .map(|fraction| match *fraction {
                0.0 => first_span,
                1.0 => last_span,
                fraction => first_span + span * fraction,
            })
            .filter(|boundary| *boundary > first[axis] && *boundary < second[axis])
            .chain(core::iter::once(second[axis]))
        {
            proxies.push(
                HybridProxy::try_new(geometry, [start, end], orientation, anchor)
                    .map_err(AircraftDefinitionError::Hybrid)?,
            );
            start = end;
        }
    }
    storage.proxies = proxies;
    Ok(storage)
}

#[cfg(test)]
mod tests;
