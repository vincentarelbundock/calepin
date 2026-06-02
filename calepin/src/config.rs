#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub dpi: Option<f64>,
    pub figure: Option<FigureConfig>,
    pub execute: Option<ExecuteConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct FigureConfig {
    pub fig_width: Option<f64>,
    pub fig_height: Option<f64>,
    pub fig_asp: Option<f64>,
    pub device: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecuteConfig {
    pub eval: Option<bool>,
    pub warning: Option<bool>,
    pub message: Option<bool>,
}
