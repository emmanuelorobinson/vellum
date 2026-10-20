//! Brand constants. Vellum is a working codename: rename here and in `@vellum/config`, nowhere else.

/// The product name.
pub const NAME: &str = "Vellum";

/// The canonical tagline.
pub const TAGLINE: &str = "Design where your code lives.";

/// The native credential namespace. Preserve or migrate it when renaming the product.
pub const KEYCHAIN_SERVICE: &str = "vellum";

/// A trusted local recipient import marks its filesystem root without inventing a Git repository.
pub const SHARE_ROOT_MARKER: &str = ".vellum-share-root";
