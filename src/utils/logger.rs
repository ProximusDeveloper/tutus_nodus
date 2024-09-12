use chrono::Local;
use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use log::LevelFilter;

pub fn setup_logger(level: LevelFilter) {
    let colors_line = ColoredLevelConfig::new()
        .error(Color::BrightRed)
        .warn(Color::BrightYellow)
        .info(Color::BrightGreen)
        .debug(Color::Magenta)
        .trace(Color::BrightCyan);

    let log_file_name = format!("./logs/logs_{}.log", Local::now().format("%d_%m_%y"));

    let log_file = fern::log_file(&log_file_name).unwrap();

    Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} {} [{}] {}",
                Local::now().format("[%Y-%m-%d][%H:%M:%S:%3f]"),
                colors_line.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(level)
        .chain(std::io::stdout())
        .chain(log_file)
        .apply()
        .unwrap();
}

#[cfg(test)]
mod tests {
    use chrono::Local;
    use fern::colors::{Color, ColoredLevelConfig};
    use fern::Dispatch;
    use log::{info, LevelFilter};
    use std::io::Write;
    use std::str;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_logger() {
        // Create a buffer to capture logs
        let log_output = Arc::new(Mutex::new(Vec::new()));

        // Set up the logger
        let level = LevelFilter::Info;
        let colors_line = ColoredLevelConfig::new()
            .error(Color::BrightRed)
            .warn(Color::BrightYellow)
            .info(Color::BrightGreen)
            .debug(Color::Magenta)
            .trace(Color::BrightCyan);

        let log_output_clone = Arc::clone(&log_output);
        Dispatch::new()
            .format(move |_out, message, record| {
                let mut buffer = log_output_clone.lock().unwrap();
                writeln!(
                    buffer,
                    "{} {} [{}] {}",
                    Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    colors_line.color(record.level()),
                    record.target(),
                    message
                )
                .unwrap();
            })
            .level(level)
            .chain(std::io::stdout())
            .apply()
            .unwrap();

        // Generate a test log message
        info!("This is a test log message");

        // Check the contents of the buffer
        let log_output = log_output.lock().unwrap();
        let log_content = str::from_utf8(&log_output).unwrap();
        assert!(log_content.contains("INFO"));
        assert!(log_content.contains("This is a test log message"));
    }
}
