use birdman_game_format::{AssistanceLevel, DifficultyPreset, InformationLevel, WeatherClass};

pub(super) fn preset(preset: DifficultyPreset) -> (&'static str, u32, &'static str) {
    match preset {
        DifficultyPreset::Beginner => ("初心者", 0, "情報と支援を充実"),
        DifficultyPreset::Standard => ("標準", 1, "標準の飛行条件"),
        DifficultyPreset::Expert => ("上級", 2, "支援を抑えた条件"),
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
        AssistanceLevel::Strong => ("自動操縦（Automatic FBW）", 0, "姿勢を自動安定"),
        AssistanceLevel::Assisted => ("強い支援（Assisted FBW）", 1, "強い姿勢安定支援"),
        AssistanceLevel::Light => ("軽い支援（Light Assisted FBW）", 2, "軽い姿勢安定支援"),
        AssistanceLevel::Manual => ("手動（Manual）", 3, "操縦支援なし"),
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

pub(super) const FLIGHT_CONTROL_INSTRUCTIONS: &str = "機首を上げる / 下げる ↑ / ↓\n左 / 右へ旋回 ← / →\n重心を前 / 後へ移動 J / L（離すと位置を保持）\nゲームパッド: スティックで操縦・重心移動\n飛行中は「一時停止」から設定や操作方法を確認できる。";
