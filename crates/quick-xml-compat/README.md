# quick-xml compatibility package

This private workspace package preserves the `quick-xml 0.39` dependency
contract required by `zbus_xml 5.1.1` while re-exporting the security-fixed
`quick-xml 0.41` implementation.

The package contains no copied upstream source. Its compatibility version still
matches the affected RustSec version range, so `deny.toml` narrowly ignores the
two corresponding advisory identifiers. The replacement must be removed when
all upstream dependencies accept `quick-xml 0.41` or newer.
