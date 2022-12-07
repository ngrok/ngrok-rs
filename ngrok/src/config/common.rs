use std::collections::HashMap;

use prost::bytes;

pub use crate::internals::proto::ProxyProto;
use crate::internals::proto::{
    gen::middleware_configuration::{
        IpRestriction,
        MutualTls,
    },
    BindExtra,
    BindOpts,
};

pub(crate) const FORWARDS_TO: &str = "rust";

/// Tunnel configuration trait, implemented by our top-level config objects.
///
/// "Sealed," i.e. not implementable outside of the crate.
pub trait TunnelConfig {
    /// The "forwards to" metadata.
    ///
    /// Only for display/informational purposes.
    fn forwards_to(&self) -> String;
    /// Internal-only, extra data sent when binding a tunnel.
    fn extra(&self) -> BindExtra;
    /// The protocol for this tunnel.
    fn proto(&self) -> String;
    /// The middleware and other configuration options for this tunnel.
    fn opts(&self) -> Option<BindOpts>;
    /// The labels for this tunnel.
    fn labels(&self) -> HashMap<String, String>;
}

// delegate references
impl<'a, T> TunnelConfig for &'a mut T
where
    T: TunnelConfig,
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

/// Restrictions placed on the origin of incoming connections to the edge.
#[derive(Clone, Default)]
pub struct CidrRestrictions {
    /// Rejects connections that do not match the given CIDRs
    pub(crate) allowed: Vec<String>,
    /// Rejects connections that match the given CIDRs and allows all other CIDRs.
    pub(crate) denied: Vec<String>,
}

impl CidrRestrictions {
    pub(crate) fn allow(&mut self, cidr: impl Into<String>) {
        self.allowed.push(cidr.into());
    }
    pub(crate) fn deny(&mut self, cidr: impl Into<String>) {
        self.denied.push(cidr.into());
    }
}

// Common
#[derive(Default)]
pub(crate) struct CommonOpts {
    // Restrictions placed on the origin of incoming connections to the edge.
    pub(crate) cidr_restrictions: CidrRestrictions,
    // The version of PROXY protocol to use with this tunnel, zero if not
    // using.
    pub(crate) proxy_proto: ProxyProto,
    // Tunnel-specific opaque metadata. Viewable via the API.
    pub(crate) metadata: Option<String>,
    // Tunnel backend metadata. Viewable via the dashboard and API, but has no
    // bearing on tunnel behavior.
    pub(crate) forwards_to: Option<String>,
}

impl CommonOpts {
    // Get the proto version of cidr restrictions
    pub(crate) fn ip_restriction(&self) -> Option<IpRestriction> {
        (!self.cidr_restrictions.allowed.is_empty() || !self.cidr_restrictions.denied.is_empty())
            .then_some(self.cidr_restrictions.clone().into())
    }
}

// transform into the wire protocol format
impl From<CidrRestrictions> for IpRestriction {
    fn from(cr: CidrRestrictions) -> Self {
        IpRestriction {
            allow_cidrs: cr.allowed,
            deny_cidrs: cr.denied,
        }
    }
}

impl From<&[bytes::Bytes]> for MutualTls {
    fn from(b: &[bytes::Bytes]) -> Self {
        let mut aggregated = Vec::new();
        b.iter().for_each(|c| aggregated.extend(c));
        MutualTls {
            mutual_tls_ca: aggregated,
        }
    }
}
