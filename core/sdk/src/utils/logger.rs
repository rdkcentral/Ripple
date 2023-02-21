pub fn init_logger(name: String) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}][{}][{}]-{}",
                chrono::Local::now().format("%Y-%m-%d-%H:%M:%S.%3f"),
                std::thread::current().name().unwrap_or("none"),
                record.level(),
                record.target(),
                name,
                message
            ))
        })
        .level(log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}
