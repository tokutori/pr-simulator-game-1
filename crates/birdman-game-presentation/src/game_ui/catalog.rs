use birdman_game_format::{AssistanceLevel, DifficultyPreset, InformationLevel, WeatherClass};

pub(super) fn preset(preset: DifficultyPreset) -> (&'static str, u32, &'static str) {
    match preset {
        DifficultyPreset::Beginner => ("初心者", 0, "情報と支援を充実"),
        DifficultyPreset::Standard => ("標準", 1, "標準の飛行条件"),
        DifficultyPreset::Expert => ("上級", 2, "最小限の計器で操縦"),
        DifficultyPreset::Realistic => ("実機寄り", 3, "手動で操縦"),
        DifficultyPreset::Custom => ("カスタム", 4, "プリセットから変更済み"),
    }
}

pub(super) fn information(level: InformationLevel) -> (&'static str, u32, &'static str) {
    match level {
        InformationLevel::Full => ("すべて（Full）", 0, "計器をすべて表示"),
        InformationLevel::Standard => ("主要計器（Standard）", 1, "主要計器を表示"),
        InformationLevel::Minimal => ("最小限（Minimal）", 2, "高度・距離・時間"),
        InformationLevel::Realistic => ("実機寄り（Realistic）", 3, "実機に近い表示"),
        InformationLevel::Custom => ("個別設定（Custom）", 4, "計器を個別選択"),
    }
}

pub(super) fn assistance(level: AssistanceLevel) -> (&'static str, u32, &'static str) {
    match level {
        AssistanceLevel::Strong => ("自動", 0, "制御器の指令を尾翼へ反映"),
        AssistanceLevel::Assisted => ("混合", 1, "入力と制御器の指令を混合"),
        AssistanceLevel::Light => ("軽い支援（旧設定）", 2, "記録時の支援区分"),
        AssistanceLevel::Manual => ("手動", 3, "入力を尾翼へ反映"),
    }
}

pub(super) fn weather(weather: WeatherClass) -> (&'static str, u32, &'static str) {
    match weather {
        WeatherClass::Calm => ("無風（Calm）", 0, "風のない条件"),
        WeatherClass::Mild => ("穏やか（Mild）", 1, "弱い風"),
        WeatherClass::Typical => ("標準（Typical）", 2, "平年値を参考"),
        WeatherClass::Challenging => ("難しい（Challenging）", 3, "操縦が難しい風"),
        WeatherClass::NearLimit => ("強風（Near Limit）", 4, "強い風に注意"),
    }
}

pub(super) fn flight_control_instructions(invert_pitch: bool) -> String {
    let pitch = if invert_pitch {
        "機首を上げる ↓ / S、下げる ↑ / W（上下軸反転）"
    } else {
        "機首を上げる ↑ / W、下げる ↓ / S"
    };
    format!(
        "{pitch}\n左へ旋回 ← / A、右へ旋回 → / D\nパイロットを前へ L / 後へ J（離すと位置を保持）\nゲームパッド: スティックで操縦・パイロット移動\nVRコントローラー: トリガーで選択・グリップで戻る・スティック上下でスクロール"
    )
}
