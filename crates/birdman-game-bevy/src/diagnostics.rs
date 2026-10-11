use birdman_game_core::{
    AeroError, AerodynamicEvaluationError, DynamicsError, HybridError, HybridLimit, HybridSite,
    HybridSurfaceRole, LoadError, SessionEndReason, TailFlightTickError, WindError,
};

pub(crate) fn result_reason_label(reason: SessionEndReason) -> &'static str {
    match reason {
        SessionEndReason::WaterContact => "着水した",
        SessionEndReason::OutOfValidEnvelope => "モデルの適用範囲を超えたため終了した",
        SessionEndReason::ManualAbort => "操作により飛行を終了した",
        SessionEndReason::FatalSimulationError => "計算処理のエラーにより終了した",
        SessionEndReason::TimeLimit => "飛行時間の上限に達した",
    }
}

pub(crate) fn failure_summary(failure: Option<TailFlightTickError>) -> &'static str {
    let dynamics = match failure {
        Some(TailFlightTickError::Dynamics(error)) => error,
        Some(_) => return "計算処理を継続できなかった。詳細は技術情報で確認できる。",
        None => return "",
    };
    match dynamics {
        DynamicsError::Load(LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error))) => {
            hybrid_summary(error)
        }
        DynamicsError::Load(LoadError::Aerodynamic(error)) => match error.cause() {
            AeroError::OutsideEnvelope => "空力モデルの計算可能範囲を超えた。",
            AeroError::Wind(WindError::OutsideGrid) => "機体が風データの有効領域を離れた。",
            _ => "空力の計算を継続できなかった。詳細は技術情報で確認できる。",
        },
        _ => "機体の運動計算を継続できなかった。詳細は技術情報で確認できる。",
    }
}

fn hybrid_summary(error: HybridError) -> &'static str {
    if error.cause() == AeroError::Wind(WindError::OutsideGrid) {
        return "機体が風データの有効領域を離れた。";
    }
    match error.limit() {
        Some(HybridLimit::ControlledAlphaDifference) => match error.site() {
            HybridSite::Surface(HybridSurfaceRole::HorizontalTail)
            | HybridSite::Proxy {
                surface: HybridSurfaceRole::HorizontalTail,
                ..
            } => "水平尾翼の局所迎角差と取付角の合計が、モデルの計算可能範囲を超えた。",
            HybridSite::Surface(HybridSurfaceRole::VerticalTail)
            | HybridSite::Proxy {
                surface: HybridSurfaceRole::VerticalTail,
                ..
            } => "垂直尾翼の局所迎角差と取付角の合計が、モデルの計算可能範囲を超えた。",
            _ => "尾翼の局所迎角差と取付角の合計が、モデルの計算可能範囲を超えた。",
        },
        Some(HybridLimit::StaticAlpha) => "機体の迎角が、空力モデルの計算可能範囲を超えた。",
        Some(HybridLimit::GlobalBeta) => "横滑り角が、空力モデルの計算可能範囲を超えた。",
        Some(HybridLimit::ElevatorIncidence) => {
            "水平尾翼の取付角が、モデルの計算可能範囲を超えた。"
        }
        Some(HybridLimit::RudderIncidence) => "垂直尾翼の取付角が、モデルの計算可能範囲を超えた。",
        Some(_) => "局所的な気流が、空力モデルの計算可能範囲を超えた。",
        None => "空力の計算を継続できなかった。詳細は技術情報で確認できる。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_core::AerodynamicStage;

    #[test]
    fn all_end_reasons_have_user_facing_labels() {
        for reason in [
            SessionEndReason::WaterContact,
            SessionEndReason::OutOfValidEnvelope,
            SessionEndReason::ManualAbort,
            SessionEndReason::FatalSimulationError,
            SessionEndReason::TimeLimit,
        ] {
            let label = result_reason_label(reason);
            assert!(!label.is_empty());
            assert!(!label.contains(&format!("{reason:?}")));
        }
    }

    #[test]
    fn controlled_tail_failure_uses_typed_site_and_preserves_original_diagnostics() {
        for (surface, expected) in [
            (HybridSurfaceRole::HorizontalTail, "水平尾翼"),
            (HybridSurfaceRole::VerticalTail, "垂直尾翼"),
        ] {
            let error = HybridError::try_from_recorded(
                HybridSite::Proxy { surface, index: 0 },
                AeroError::OutsideEnvelope,
                Some(HybridLimit::ControlledAlphaDifference),
                Some(AerodynamicStage::First),
            )
            .unwrap();
            let failure = TailFlightTickError::Dynamics(DynamicsError::Load(
                LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
            ));
            let summary = failure_summary(Some(failure));
            assert!(summary.contains(expected));
            assert!(summary.contains("局所迎角差と取付角"));
            assert!(!summary.contains("Some("));
            assert!(!summary.contains("HybridError"));
            assert!(format!("{failure:?}").contains("ControlledAlphaDifference"));
            assert_eq!(failure.end_reason(), SessionEndReason::OutOfValidEnvelope);
        }
    }

    #[test]
    fn successful_ending_has_no_failure_explanation() {
        assert_eq!(failure_summary(None), "");
    }
}
