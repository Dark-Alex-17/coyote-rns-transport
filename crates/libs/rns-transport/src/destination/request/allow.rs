use std::collections::HashSet;

use crate::hash::AddressHash;

/// Who a registered request handler answers: `ALLOW_NONE = 0x00`,
/// `ALLOW_ALL = 0x01`, `ALLOW_LIST = 0x02` (`RNS/Destination.py:74-77`).
///
/// `List` is matched against the identity the peer delivered with
/// `LINKIDENTIFY` (`RNS/Link.py:821`); a link whose peer never identified
/// is denied silently, as is a denied peer (`RNS/Link.py:821,855`).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum RequestAllow {
    /// `ALLOW_NONE` (`RNS/Destination.py:74`).
    None,
    /// `ALLOW_ALL` (`RNS/Destination.py:75`).
    All,
    /// `ALLOW_LIST` (`RNS/Destination.py:76`): the identities that may ask,
    /// matched against the one that identified on the link
    /// (`RNS/Link.py:821`).
    List(HashSet<AddressHash>),
}
