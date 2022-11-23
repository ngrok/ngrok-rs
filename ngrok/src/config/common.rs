use crate::internals::proto::ProxyProto;

pub(crate) const FORWARDS_TO: &str = "rust";

// Tunnel configuration trait, implemented by our top-level config objects.
// "Sealed," i.e. not implementable outside of the crate.
pub trait TunnelConfig: private::TunnelConfigPrivate {}
impl<T> TunnelConfig for T where T: private::TunnelConfigPrivate {}

// Non-exported private tunnel config type that "seals" the exported one
// This is where we'll produce the config struct that ultimately gets passed
// to the tunnel bind RPCs.
// private: https://rust-lang.github.io/api-guidelines/future-proofing.html
// avoids "A private trait was used on a public type parameter bound"
//   https://doc.rust-lang.org/error_codes/E0445.html
// or "can't leak crate-private trait"
//   https://users.rust-lang.org/t/pub-trait-in-private-module-no-cant-leak-private-trait-error/46052
pub(crate) mod private {
    use std::collections::HashMap;

    use crate::internals::proto::{
        BindExtra,
        BindOpts,
    };

    // This is the internal-only interface that all config.Tunnel implementations
    // *also* implement. This lets us pull the necessary bits out of it without
    // polluting the public interface with internal details.
    pub trait TunnelConfigPrivate {
        fn forwards_to(&self) -> String;
        fn extra(&self) -> BindExtra;
        fn proto(&self) -> String;
        fn opts(&self) -> Option<BindOpts>;
        fn labels(&self) -> HashMap<String, String>;
    }

    // delegate references
    impl<'a, T> TunnelConfigPrivate for &'a T
    where
        T: TunnelConfigPrivate,
    {
        fn forwards_to(&self) -> String {
            (**self).forwards_to()
        }
        fn extra(&self) -> BindExtra {
            (**self).extra()
        }
        fn proto(&self) -> String {
            (**self).proto()
        }
        fn opts(&self) -> Option<BindOpts> {
            (**self).opts()
        }
        fn labels(&self) -> HashMap<String, String> {
            (**self).labels()
        }
    }

    // delegate mutable references
    impl<'a, T> TunnelConfigPrivate for &'a mut T
    where
        T: TunnelConfigPrivate,
    {
        fn forwards_to(&self) -> String {
            (**self).forwards_to()
        }
        fn extra(&self) -> BindExtra {
            (**self).extra()
        }
        fn proto(&self) -> String {
            (**self).proto()
        }
        fn opts(&self) -> Option<BindOpts> {
            (**self).opts()
        }
        fn labels(&self) -> HashMap<String, String> {
            (**self).labels()
        }
    }
}

pub struct CidrRestrictions {
    // todo
}

// Common
pub(crate) struct CommonOpts {
    // Restrictions placed on the origin of incoming connections to the edge.
    pub(crate) cidr_restrictions: Option<CidrRestrictions>,
    // The version of PROXY protocol to use with this tunnel, zero if not
    // using.
    pub(crate) proxy_proto: Option<ProxyProto>,
    // Tunnel-specific opaque metadata. Viewable via the API.
    pub(crate) metadata: Option<String>,
    // Tunnel backend metadata. Viewable via the dashboard and API, but has no
    // bearing on tunnel behavior.
    pub(crate) forwards_to: Option<String>,
}

impl Default for CommonOpts {
    fn default() -> Self {
        CommonOpts {
            cidr_restrictions: None,
            proxy_proto: None,
            metadata: None,
            forwards_to: None,
        }
    }
}
