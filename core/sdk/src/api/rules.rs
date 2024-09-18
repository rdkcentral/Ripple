pub enum InternalRule {
    NotificationServiceOnConnect,
    AnalyticsProcessor,
}

impl ToString for InternalRule {
    fn to_string(&self) -> String {
        let suffix = match self {
            InternalRule::NotificationServiceOnConnect => {
                String::from("notification_service_on_connect")
            }
            InternalRule::AnalyticsProcessor => String::from("analytics_processor"),
        };

        format!("internal.{}", suffix)
    }
}
