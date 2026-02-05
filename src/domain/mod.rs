mod registration;
pub mod value_objects;

pub use registration::DomainRegistration;
pub use value_objects::{DomainName, PathPrefix, ProxyTarget, Route, RouteTarget};
