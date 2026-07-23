pub trait ServiceInfo {
    fn get_service_name(&self) -> &str;
    fn get_service_version(&self) -> &str;
}
