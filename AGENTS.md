# Roxy - Claude Development Guide

## Project Overview

Roxy is a local development proxy tool written in Rust that enables developers to run multiple projects with custom `.roxy` domains and automatic HTTPS support. Think Laravel Valet, but written in Rust.

**Philosophy**: Pragmatic, idiomatic Rust. Build the simplest thing that works. YAGNI principles apply throughout.

## Rust Skills (AI)

This repo vendors `rust-skills` (skills-only, no MCP) at `.rust-skills/skills/`.

Local setup links those skills into:
- `~/.codex/skills/` (Codex)
- `~/.claude/skills/` (Claude Code)

When working on Rust tasks, prefer:
- Invoke `rust-router` first for Rust questions/errors/design.
- Then follow the routed skill (`m01-*`..`m07-*`, `m09-*`..`m15-*`, `domain-*`).
- Use `unsafe-checker` for any unsafe/FFI review.


## Core Principles

### 1. Idiomatic Rust

- Use standard library types and patterns
- Embrace `Result<T, E>` for error handling - no panics in library code
- Use `Option<T>` appropriately - avoid unnecessary unwrapping
- Prefer iterators over loops where it improves clarity
- Use enums for state and behavior variants
- Implement standard traits (`Display`, `Debug`, `From`, etc.) where appropriate
- Follow Rust naming conventions (snake_case for functions/variables, PascalCase for types)

### 2. YAGNI (You Aren't Gonna Need It)

- **Don't build features that aren't in REQUIREMENTS.md**
- Don't create abstractions until you need them in multiple places
- Don't add configuration options until someone asks for them
- Don't optimize until there's a proven performance issue
- Don't add plugins, hooks, or extensibility mechanisms in v1
- Start with simple implementations - refactor when complexity demands it

### 3. Error Handling

- Use `anyhow::Result` for application-level errors (CLI commands)
- Use custom error types (with `thiserror`) only when you need to match on error variants
- Provide helpful error messages - include context about what failed and why
- Never use `.unwrap()` or `.expect()` in production code paths
- Use `?` operator liberally for clean error propagation

### 4. Architecture Guidelines

#### Keep It Simple

- Start with a single binary with multiple modules
- Don't split into multiple crates unless there's a clear benefit
- Avoid over-abstraction - traits should solve real problems, not theoretical ones
- Direct implementation beats clever generics

#### Module Structure

```
roxy/
├── src/
│   ├── main.rs           # CLI entry point, argument parsing
│   ├── cli/              # CLI commands implementation
│   │   ├── mod.rs
│   │   ├── install.rs
│   │   ├── register.rs
│   │   ├── unregister.rs
│   │   └── list.rs
│   ├── daemon/           # HTTP server and proxy logic
│   │   ├── mod.rs
│   │   ├── server.rs
│   │   └── router.rs
│   ├── dns/              # DNS configuration management
│   │   └── mod.rs
│   ├── certs/            # Certificate generation and management
│   │   └── mod.rs
│   ├── config/           # Configuration storage and loading
│   │   └── mod.rs
│   └── lib.rs            # Library root (shared types and utilities)
```

#### Dependencies Philosophy

- **Minimize dependencies** - each new crate is a maintenance burden
- Prefer well-maintained, popular crates over niche ones
- Read the code of small crates before adding them
- Don't add a dependency to save 20 lines of code

### 5. Domain-Driven Design (Lightweight)

Use DDD concepts to make the code expressive and clear, but avoid DDD ceremony and over-engineering.

#### Value Objects

Wrap primitives in meaningful types to prevent mistakes and make the domain explicit:

```rust
// Value objects - immutable, validated on construction
pub struct DomainName(String);
pub struct Port(u16);

impl DomainName {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        if !name.ends_with(".local") {
            return Err(anyhow!("Domain must end with .local"));
        }
        if name.len() < 7 {  // Minimum: "a.local"
            return Err(anyhow!("Domain name too short"));
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for DomainName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

**When to use Value Objects:**

- Primitives with validation rules (DomainName, Port)
- Concepts that are compared by value (Certificate, FilePath)
- Types that should be immutable

**When NOT to use Value Objects:**

- Don't wrap primitives that have no validation or domain meaning
- Don't create value objects just to have them

#### Entities

Types with identity that can change over time:

```rust
// Entity - has identity (domain name), mutable state
pub struct DomainRegistration {
    domain: DomainName,
    target: Target,
    https_enabled: bool,
    created_at: SystemTime,
}

impl DomainRegistration {
    pub fn new(domain: DomainName, target: Target) -> Self {
        Self {
            domain,
            target,
            https_enabled: false,
            created_at: SystemTime::now(),
        }
    }

    pub fn enable_https(&mut self) {
        self.https_enabled = true;
    }

    pub fn domain(&self) -> &DomainName {
        &self.domain
    }
}
```

**Key points:**

- Entities have identity - two domains with same name are the same domain
- Can have mutable state with methods that enforce invariants
- Keep entity methods focused on domain logic, not persistence

#### Domain Enums

Use enums to model domain concepts and behavior:

```rust
pub enum Target {
    StaticFiles(PathBuf),
    ReverseProxy(Port),
}

impl Target {
    pub fn validate(&self) -> Result<()> {
        match self {
            Target::StaticFiles(path) => {
                if !path.exists() {
                    return Err(anyhow!("Path does not exist: {}", path.display()));
                }
                Ok(())
            }
            Target::ReverseProxy(port) => {
                if port.0 < 1024 {
                    return Err(anyhow!("Port must be >= 1024"));
                }
                Ok(())
            }
        }
    }
}
```

#### Domain Services

Operations that don't naturally belong to a single entity:

```rust
// Domain service - coordinates domain operations
pub struct CertificateService {
    cert_dir: PathBuf,
}

impl CertificateService {
    /// Generates a self-signed certificate for the given domain
    pub fn generate_certificate(&self, domain: &DomainName) -> Result<Certificate> {
        // Certificate generation logic
        // This doesn't belong on DomainName or Certificate entities
    }

    pub fn install_to_system_trust(&self, cert: &Certificate) -> Result<()> {
        // System integration logic
    }
}
```

**When to use Domain Services:**

- Operations involving multiple entities
- Operations requiring external dependencies (filesystem, system calls)
- Complex domain logic that doesn't fit naturally on an entity

**Keep them focused:**

- One service per domain concern (CertificateService, DnsService)
- Methods should be about domain operations, not infrastructure

#### Repositories

Abstract persistence without coupling domain logic to storage:

```rust
// Repository trait - only if you need multiple implementations
pub trait DomainRepository {
    fn save(&mut self, registration: DomainRegistration) -> Result<()>;
    fn find(&self, domain: &DomainName) -> Result<Option<DomainRegistration>>;
    fn find_all(&self) -> Result<Vec<DomainRegistration>>;
    fn delete(&mut self, domain: &DomainName) -> Result<()>;
}

// Simple implementation using JSON file
pub struct JsonDomainRepository {
    config_path: PathBuf,
}

impl DomainRepository for JsonDomainRepository {
    fn save(&mut self, registration: DomainRegistration) -> Result<()> {
        // Load, update, save config.json
    }
    // ...
}
```

**Important: Keep it simple**

- Only create a Repository trait if you actually need multiple implementations
- If you only have one storage mechanism (JSON file), just use a concrete struct
- Don't create a repository for every entity - only for aggregates

```rust
// Simpler version without trait (prefer this unless you need abstraction)
pub struct ConfigStore {
    config_path: PathBuf,
}

impl ConfigStore {
    pub fn save_domain(&mut self, registration: DomainRegistration) -> Result<()> {
        // Direct implementation
    }
}
```

#### Application Services / Use Cases

Orchestrate domain operations for specific use cases:

```rust
// Application service - orchestrates use case
pub struct RegisterDomainUseCase {
    config_store: ConfigStore,
    cert_service: CertificateService,
    dns_service: DnsService,
}

impl RegisterDomainUseCase {
    pub fn execute(&mut self, domain: DomainName, target: Target) -> Result<()> {
        // 1. Validate domain isn't already registered
        if self.config_store.find_domain(&domain)?.is_some() {
            return Err(anyhow!("Domain already registered: {}", domain));
        }

        // 2. Validate target
        target.validate()?;

        // 3. Create registration
        let mut registration = DomainRegistration::new(domain.clone(), target);

        // 4. Generate certificate
        let cert = self.cert_service.generate_certificate(&domain)?;
        self.cert_service.install_to_system_trust(&cert)?;
        registration.enable_https();

        // 5. Save configuration
        self.config_store.save_domain(registration)?;

        Ok(())
    }
}
```

**Use application services to:**

- Coordinate multiple domain services
- Enforce business rules across entities
- Handle the full workflow of a use case
- Keep CLI commands thin - they just call use cases

#### Proposed Module Structure with DDD

```
roxy/
├── src/
│   ├── main.rs                    # CLI entry point
│   ├── cli/                       # CLI layer - thin, delegates to use cases
│   │   ├── mod.rs
│   │   ├── commands/
│   │   │   ├── install.rs
│   │   │   ├── register.rs
│   │   │   ├── unregister.rs
│   │   │   └── list.rs
│   │   └── args.rs                # Clap argument definitions
│   │
│   ├── domain/                    # Domain layer - core business logic
│   │   ├── mod.rs
│   │   ├── entities/
│   │   │   └── domain_registration.rs
│   │   ├── value_objects/
│   │   │   ├── domain_name.rs
│   │   │   ├── port.rs
│   │   │   ├── target.rs
│   │   │   └── certificate.rs
│   │   └── services/
│   │       ├── certificate_service.rs
│   │       └── dns_service.rs
│   │
│   ├── application/               # Application layer - use cases
│   │   ├── mod.rs
│   │   ├── register_domain.rs
│   │   ├── unregister_domain.rs
│   │   └── list_domains.rs
│   │
│   ├── infrastructure/            # Infrastructure layer - I/O, persistence
│   │   ├── mod.rs
│   │   ├── config_store.rs        # JSON persistence
│   │   ├── system/
│   │   │   ├── macos.rs          # macOS-specific DNS/cert code
│   │   │   └── linux.rs          # Linux-specific DNS/cert code
│   │   └── daemon/
│   │       ├── server.rs         # HTTP server
│   │       └── router.rs         # Request routing
│   │
│   └── lib.rs
```

#### DDD Anti-Patterns to Avoid

❌ **Don't create layers just to have layers:**

```rust
// Bad - unnecessary abstraction
trait DomainNameValidator {
    fn validate(&self, name: &str) -> Result<()>;
}

// Good - validation in constructor
impl DomainName {
    pub fn new(name: String) -> Result<Self> {
        // validation here
    }
}
```

❌ **Don't create repositories for everything:**

```rust
// Bad - repository for value object
trait CertificateRepository {
    fn find_certificate(&self, domain: &DomainName) -> Result<Certificate>;
}

// Good - certificates are managed by CertificateService
impl CertificateService {
    pub fn get_certificate(&self, domain: &DomainName) -> Result<Certificate> {
        // Load from filesystem directly
    }
}
```

❌ **Don't use events if you don't need them:**

```rust
// Bad - overkill for simple app
pub struct DomainRegisteredEvent {
    domain: DomainName,
    timestamp: SystemTime,
}
pub trait EventBus {
    fn publish(&self, event: DomainRegisteredEvent);
}

// Good - direct function calls
pub fn register_domain(...) -> Result<()> {
    // Just do the work
}
```

❌ **Don't create complex aggregate rules:**

```rust
// Bad - unnecessary complexity
pub struct DomainAggregate {
    root: DomainRegistration,
    certificates: Vec<Certificate>,
    dns_records: Vec<DnsRecord>,
}

// Good - simple relationships
pub struct DomainRegistration {
    domain: DomainName,
    target: Target,
    https_enabled: bool,
}
// Certificates and DNS are managed by services
```

#### DDD Guidelines Summary

**DO:**

- ✅ Use value objects for validated primitives (DomainName, Port)
- ✅ Use entities for things with identity and lifecycle
- ✅ Use enums to model domain variants (Target, Status)
- ✅ Create domain services for operations not belonging to entities
- ✅ Use application services to orchestrate use cases
- ✅ Make domain types expressive with good method names
- ✅ Keep domain layer free of infrastructure concerns

**DON'T:**

- ❌ Create abstractions without concrete need
- ❌ Use repository pattern unless you need multiple implementations
- ❌ Add events, sagas, or complex aggregate rules
- ❌ Create DTOs unless crossing clear boundaries (we rarely need them in Rust)
- ❌ Separate domain and persistence models (Rust's type system makes this less necessary)

**Remember:** DDD should make code MORE clear, not more complex. If a DDD pattern adds complexity without clear benefit, skip it.

### 6. What NOT to Do

❌ **Don't over-engineer:**

- No custom allocators or unsafe code (unless absolutely necessary)
- No macros for things functions can do
- No trait hierarchies more than 2 levels deep
- No generic programming for a single use case
- No async where blocking I/O works fine (file operations)

❌ **Don't add unnecessary features:**

- No plugin system in v1
- No web UI in v1
- No remote configuration
- No cloud sync
- No analytics or telemetry

❌ **Don't optimize prematurely:**

- Simple `String` is fine over `Cow<str>` until proven otherwise
- `.clone()` is acceptable for small structs and infrequent operations
- HashMap is fine before trying FxHashMap or BTreeMap
- Standard threading is fine before considering rayon or custom thread pools

### 7. Dependencies

#### Required Core Dependencies

```toml
# Async runtime (daemon needs async for HTTP server)
tokio = { version = "1", features = ["full"] }

# HTTP server and client
axum = "0.7"           # Simple, ergonomic web framework
hyper = "1.0"          # HTTP primitives (used by axum)
tower = "0.4"          # Middleware (used by axum)

# TLS/SSL
rustls = "0.23"        # Pure Rust TLS
rcgen = "0.12"         # Certificate generation

# CLI
clap = { version = "4", features = ["derive"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
anyhow = "1.0"         # Application errors
thiserror = "1.0"      # Library errors (if needed)
```

#### Consider Only When Needed

- `tracing` / `tracing-subscriber` - if logging needs become complex
- `notify` - if we need file watching
- `daemonize` - if simple fork isn't enough

### 8. Code Style

#### Formatting

- Use `rustfmt` with default settings
- Run `cargo fmt` before committing
- Max line length: 100 characters (rustfmt default)

#### Documentation

- Public functions must have doc comments
- Include examples in doc comments for non-obvious functions
- Explain *why*, not *what* - the code shows what

```rust
// Good
/// Registers a new domain with either a file path or port.
///
/// Returns an error if the domain is already registered or if
/// certificate generation fails.
pub fn register_domain(domain: &str, target: Target) -> Result<()> {
    // ...
}

// Bad - just repeating what the signature says
/// Registers a domain
pub fn register_domain(domain: &str, target: Target) -> Result<()> {
    // ...
}
```

#### Function Size

- Keep functions under 50 lines when possible
- Extract helper functions when logic becomes nested 3+ levels
- A function should do one thing

#### Types Over Primitives

```rust
// Good - clear intent
pub struct Domain(String);
pub enum Target {
    Path(PathBuf),
    Port(u16),
}

// Bad - unclear what the string and u16 mean
pub fn register(domain: String, target: Either<String, u16>) -> Result<()>
```

### 9. Testing Strategy

#### Unit Tests

- Test business logic, not trivial getters/setters
- Use `#[cfg(test)]` modules in the same file
- Mock external dependencies (filesystem, network) in tests
- Test error cases, not just happy paths

#### Integration Tests

- Put in `tests/` directory
- Test actual CLI commands end-to-end
- Use temporary directories for filesystem tests

#### What NOT to Test

- Don't test external libraries (axum, rustls, etc.)
- Don't test trivial constructors or accessors
- Don't test private implementation details

```rust
// Good - tests behavior
#[test]
fn test_duplicate_domain_returns_error() {
    let mut config = Config::new();
    config.register("app.local", Target::Port(3000)).unwrap();
    let result = config.register("app.local", Target::Port(4000));
    assert!(result.is_err());
}

// Bad - tests implementation detail
#[test]
fn test_domains_stored_in_hashmap() {
    let config = Config::new();
    assert_eq!(config.domains.len(), 0);
}
```

### 10. Error Messages

Make errors actionable and friendly:

```rust
// Good
return Err(anyhow!(
    "Failed to register domain '{}': domain already exists.\n\
     Use 'roxy unregister {}' first, or choose a different domain name.",
    domain, domain
));

// Bad
return Err(anyhow!("Domain exists"));
```

### 11. Git Commits

- Use conventional commits format: `feat:`, `fix:`, `docs:`, `refactor:`, etc.
- Keep commits focused and atomic
- Write commit messages that explain *why*, not *what*

### 12. Performance Targets

Don't optimize until these are violated:

- Daemon startup: < 100ms
- Domain registration: < 500ms (including cert generation)
- Request latency overhead: < 10ms
- Memory usage (idle): < 50MB
- Memory usage (under load): < 200MB

### 13. Security Considerations

- Never log or display certificate private keys
- Validate all user inputs (domain names, paths, ports)
- Use appropriate file permissions (0600 for keys, 0644 for certs)
- Require explicit confirmation for destructive operations
- Run daemon with minimum required privileges

### 14. Platform-Specific Code

Use conditional compilation for platform differences:

```rust
#[cfg(target_os = "macos")]
fn setup_dns() -> Result<()> {
    // macOS implementation using /etc/resolver/
}

#[cfg(target_os = "linux")]
fn setup_dns() -> Result<()> {
    // Linux implementation using dnsmasq
}
```

Start with macOS support only. Add Linux when macOS is working.

## Development Workflow

1. **Before starting a feature:**
   - Read the relevant section in REQUIREMENTS.md
   - Understand what success looks like
   - Choose the simplest approach

2. **While implementing:**
   - Write the minimal code that satisfies the requirement
   - Add error handling as you go
   - Add tests for non-trivial logic
   - Run `cargo check` frequently

3. **Before committing:**
   - Run `cargo fmt`
   - Run `cargo clippy -- -D warnings`
   - Run `cargo test`
   - Test manually if it's user-facing functionality

4. **Code review checklist:**
   - Is this the simplest solution?
   - Are error messages helpful?
   - Are edge cases handled?
   - Is it idiomatic Rust?
   - Does it follow YAGNI?

## When to Refactor

Refactor when you see:

- Same code pattern repeated 3+ times
- Functions longer than 50 lines doing multiple things
- Deep nesting (3+ levels)
- Hard-to-test code due to tight coupling
- Actual performance problems (not theoretical)

Don't refactor:

- Code that's "not pretty" but works fine
- To make it "more generic" without a concrete second use case
- To match patterns from other projects
- Because you thought of a "better" way (unless current way has issues)

## Questions to Ask

Before adding complexity, ask:

1. Does this solve a real problem today?
2. Is there a simpler way?
3. What's the cost of doing nothing?
4. Can we do this later if needed?

## Remember

> "Perfection is achieved, not when there is nothing more to add, but when there is nothing left to take away." - Antoine de Saint-Exupéry

Build Roxy to work reliably for its core use case. Resist the temptation to make it do everything. Shipping a simple, working tool beats architecting a flexible, unfinished framework.
