fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let config = repose_platform::AppConfig {
        window_title: "rext".into(),
        window_size: (1440, 900),
        enable_inspector: cfg!(debug_assertions),
        ..Default::default()
    };
    repose_platform::run_desktop_app_with_config(|s, rc| rext_ui::app(s, rc), config)
}
