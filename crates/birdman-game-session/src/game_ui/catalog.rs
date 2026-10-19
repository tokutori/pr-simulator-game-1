use birdman_game_format::{AssistanceLevel, DifficultyPreset, InformationLevel, WeatherClass};

pub(super) fn preset(preset: DifficultyPreset) -> (&'static str, u32, &'static str) {
    match preset {
        DifficultyPreset::Beginner => ("初心者", 0, "情報と支援を充実させた条件"),
        DifficultyPreset::Standard => ("標準", 1, "標準的な表示・支援・気象"),
        DifficultyPreset::Expert => ("上級", 2, "情報と支援を抑えた条件"),
        DifficultyPreset::Realistic => ("実機寄り", 3, "実機に近い情報量と手動操縦"),
        DifficultyPreset::Custom => ("カスタム", 4, "プリセットから変更済み"),
    }
}

pub(super) fn information(level: InformationLevel) -> (&'static str, u32, &'static str) {
    match level {
        InformationLevel::Full => ("すべて（Full）", 0, "利用可能な計器と補助表示をすべて表示"),
        InformationLevel::Standard => (
            "主要計器（Standard）",
            1,
            "速度・高度・姿勢などの主要計器を表示",
        ),
        InformationLevel::Minimal => ("最小限（Minimal）", 2, "高度・距離・時間を表示"),
        InformationLevel::Realistic => ("実機寄り（Realistic）", 3, "実機に近い情報量で表示"),
        InformationLevel::Custom => ("個別設定（Custom）", 4, "計器・補助表示を個別選択"),
    }
}

pub(super) fn assistance(level: AssistanceLevel) -> (&'static str, u32, &'static str) {
    match level {
        AssistanceLevel::Strong => ("自動操縦（Automatic FBW）", 0, "姿勢の安定を自動制御する"),
        AssistanceLevel::Assisted => (
            "強い支援（Assisted FBW）",
            1,
            "操縦入力に強い姿勢安定支援を加える",
        ),
        AssistanceLevel::Light => (
            "軽い支援（Light Assisted FBW）",
            2,
            "操縦入力に軽い姿勢安定支援を加える",
        ),
        AssistanceLevel::Manual => ("手動（Manual）", 3, "操縦入力を直接反映する"),
    }
}

pub(super) fn weather(weather: WeatherClass) -> (&'static str, u32, &'static str) {
    match weather {
        WeatherClass::Calm => ("無風（Calm）", 0, "風のない穏やかな条件"),
        WeatherClass::Mild => ("穏やか（Mild）", 1, "弱い風の条件"),
        WeatherClass::Typical => ("標準（Typical）", 2, "彦根の平年値を参考にした条件"),
        WeatherClass::Challenging => ("難しい（Challenging）", 3, "操縦が難しくなる風の条件"),
        WeatherClass::NearLimit => ("強風（Near Limit）", 4, "強い風に注意が必要な条件"),
    }
}

pub(super) const FLIGHT_CONTROL_INSTRUCTIONS: &str = "機首を上げる / 下げる ↑ / ↓\n左 / 右へ旋回 ← / →\n重心を前 / 後へ移動 J / L（離すと位置を保持）\nゲームパッド: スティックで操縦・重心移動\n飛行中は「一時停止」から設定や操作方法を確認できる。";
