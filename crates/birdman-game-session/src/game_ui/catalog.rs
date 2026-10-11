use birdman_game_format::{AssistanceLevel, DifficultyPreset, InformationLevel, WeatherClass};

pub(super) fn preset(preset: DifficultyPreset) -> (&'static str, u32, &'static str) {
    match preset {
        DifficultyPreset::Beginner => ("Beginner", 0, "情報と支援を充実させた条件"),
        DifficultyPreset::Standard => ("Standard", 1, "標準的な表示・支援・気象"),
        DifficultyPreset::Expert => ("Expert", 2, "情報と支援を抑えた条件"),
        DifficultyPreset::Realistic => ("Realistic", 3, "実機に近い情報量と手動操縦"),
        DifficultyPreset::Custom => ("Custom", 4, "プリセットから変更済み"),
    }
}

pub(super) fn information(level: InformationLevel) -> (&'static str, u32, &'static str) {
    match level {
        InformationLevel::Full => ("Full", 0, "利用可能な計器と補助表示をすべて表示"),
        InformationLevel::Standard => ("Standard", 1, "主要計器と警告を表示"),
        InformationLevel::Minimal => ("Minimal", 2, "高度・距離・時間を表示"),
        InformationLevel::Realistic => ("Realistic", 3, "実機に近い情報量で表示"),
        InformationLevel::Custom => ("Custom", 4, "計器・補助表示を個別選択"),
    }
}

pub(super) fn assistance(level: AssistanceLevel) -> (&'static str, u32, &'static str) {
    match level {
        AssistanceLevel::Strong => ("Strong / Automatic FBW", 0, "Automatic FBWで舵を制御する"),
        AssistanceLevel::Assisted => ("Assisted / Shared FBW", 1, "Shared FBWによる強い操縦支援"),
        AssistanceLevel::Light => ("Light / Shared FBW", 2, "Shared FBWによる軽い操縦支援"),
        AssistanceLevel::Manual => ("Manual", 3, "操縦入力を直接反映する"),
    }
}

pub(super) fn weather(weather: WeatherClass) -> (&'static str, u32, &'static str) {
    match weather {
        WeatherClass::Calm => ("Synthetic Calm", 0, "無風の合成シナリオ"),
        WeatherClass::Mild => ("Synthetic Mild", 1, "軽い風の合成シナリオ"),
        WeatherClass::Typical => ("Synthetic Typical", 2, "標準的な合成シナリオ"),
        WeatherClass::Challenging => ("Synthetic Challenging", 3, "難しい合成シナリオ"),
        WeatherClass::NearLimit => ("Synthetic NearLimit", 4, "強い風の合成シナリオ"),
    }
}

pub(super) const FLIGHT_CONTROL_INSTRUCTIONS: &str = "機首上げ / 下げ ↑ / ↓ · 右 / 左旋回 → / ←\n水平・垂直尾翼のincidenceを制御する。独立したroll入力はない。\n重心移動 J / L · Keyboardは解放時Hold、Gamepadは位置をSetする。";
