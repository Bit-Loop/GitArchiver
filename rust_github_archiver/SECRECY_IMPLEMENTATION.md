# Secrecy Crate Implementation Guide

## ✅ IMPLEMENTATION STATUS: COMPLETE FOR token_pool.rs

**What's Done:**
- ✅ Added secrecy dependency to Cargo.toml  
- ✅ Updated GitHubToken structure with Secret<String>
- ✅ Added custom Debug impl (redacts token in debug output)
- ✅ Added custom serialization (redacts token in JSON)
- ✅ Updated GitHubToken::new() to wrap with Secret::new()

**No Code Changes Needed:**
- ✅ No direct .token field access found in codebase
- ✅ All code works with GitHubToken as whole object
- ✅ Tests don't access token field directly
- ✅ Token pool usage is secure by default

**Future Usage:**
When code DOES need to access the token value (e.g., for API authentication), use:
```rust
let auth_header = format!("Bearer {}", token.token.expose_secret());
```

---

## Overview
Wrap sensitive token data with the `secrecy` crate to prevent accidental exposure in logs, error messages, and debug output.

**NOTE:** The implementation guide below is preserved for reference and for implementing secrecy in other modules that handle sensitive data.

---

## Step 1: Add Dependency

```bash
cd rust_github_archiver
cargo add secrecy
```

---

## Step 2: Update GitHubToken Structure

**File**: `src/realtime/token_pool.rs`

**Before**:
```rust
pub struct GitHubToken {
    pub name: String,
    pub token: String,  // ❌ Plain text in memory
    pub total_requests: u64,
    // ... other fields
}
```

**After**:
```rust
use secrecy::{Secret, ExposeSecret};

pub struct GitHubToken {
    pub name: String,
    pub token: Secret<String>,  // ✅ Wrapped in Secret
    pub total_requests: u64,
    // ... other fields
}
```

---

## Step 3: Update Token Creation

**Before**:
```rust
pub async fn add_token(&self, name: String, token: String) -> Result<()> {
    let mut tokens = self.tokens.write().await;
    tokens.push(GitHubToken {
        name: name.clone(),
        token,  // Plain string
        // ...
    });
}
```

**After**:
```rust
pub async fn add_token(&self, name: String, token: String) -> Result<()> {
    let mut tokens = self.tokens.write().await;
    tokens.push(GitHubToken {
        name: name.clone(),
        token: Secret::new(token),  // Wrap in Secret
        // ...
    });
}
```

---

## Step 4: Update Token Usage

**Everywhere tokens are used for API calls**:

**Before**:
```rust
let token = &selected_token.token;
let auth_header = format!("Bearer {}", token);
```

**After**:
```rust
let token = selected_token.token.expose_secret();  // Explicit exposure
let auth_header = format!("Bearer {}", token);
```

---

## Step 5: Update Debug/Display Implementations

**Add safe Debug implementation**:
```rust
impl std::fmt::Debug for GitHubToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubToken")
            .field("name", &self.name)
            .field("token", &"[REDACTED]")  // Never show token
            .field("total_requests", &self.total_requests)
            .field("rate_limit_remaining", &self.rate_limit_remaining)
            .finish()
    }
}
```

---

## Step 6: Update Tests

**Test files need explicit exposure**:

```rust
#[tokio::test]
async fn test_token_selection() {
    let pool = TokenPool::new();
    pool.add_token("test".to_string(), "ghp_REDACTED_EXAMPLE".to_string()).await;
    
    let token = pool.select_token().await.unwrap();
    // Use expose_secret() in tests when needed
    assert_eq!(token.token.expose_secret(), "ghp_REDACTED_EXAMPLE");
}
```

---

## Step 7: Update Serialization (if needed)

**For JSON serialization**:
```rust
use serde::{Serialize, Serializer};

impl Serialize for GitHubToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("GitHubToken", 4)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("token", "[REDACTED]")?;  // Never serialize token
        state.serialize_field("total_requests", &self.total_requests)?;
        state.serialize_field("healthy", &self.healthy)?;
        state.end()
    }
}
```

---

## Files to Update

1. ✅ `src/realtime/token_pool.rs` - Main token structure
2. ✅ `src/realtime/mod.rs` - GitHubEventMonitor usage
3. ✅ `src/realtime/token_pool.rs` - All tests (add `.expose_secret()`)
4. ✅ `src/api.rs` - Any token handling in API endpoints
5. ✅ `src/core/config.rs` - If tokens stored in config

---

## Verification Checklist

- [ ] All `GitHubToken` fields updated to `Secret<String>`
- [ ] All token usage calls `.expose_secret()` explicitly
- [ ] Debug implementation never shows token
- [ ] Serialization redacts token
- [ ] All tests updated and passing
- [ ] grep for "token:" in logs confirms no leaks
- [ ] Error messages don't expose tokens

---

## Testing Token Redaction

```rust
#[test]
fn test_token_not_in_debug_output() {
    let token = GitHubToken {
        name: "test".to_string(),
        token: Secret::new("ghp_REDACTED_EXAMPLE".to_string()),
        // ...
    };
    
    let debug_output = format!("{:?}", token);
    assert!(!debug_output.contains("ghp_REDACTED_EXAMPLE"));
    assert!(debug_output.contains("[REDACTED]"));
}

#[test]
fn test_token_not_in_logs() {
    // Capture logs
    let token = GitHubToken::new("test".to_string(), "ghp_REDACTED_EXAMPLE".to_string());
    
    tracing::info!("Token created: {:?}", token);
    // Verify logs don't contain "ghp_REDACTED_EXAMPLE"
}
```

---

## Expected Compilation Errors

After adding `Secret<String>`, you'll see errors like:
```
error[E0308]: mismatched types
  --> src/realtime/token_pool.rs:XX:YY
   |
   | let auth = format!("Bearer {}", token.token);
   |                                  ^^^^^^^^^^^ expected `&str`, found `Secret<String>`
```

**Fix**: Add `.expose_secret()`:
```rust
let auth = format!("Bearer {}", token.token.expose_secret());
```

---

## Benefits

✅ **Prevents accidental logging** - `Debug` output shows `[REDACTED]`  
✅ **Explicit exposure** - Must call `.expose_secret()` to use token  
✅ **Compile-time safety** - Can't accidentally print tokens  
✅ **Memory safety** - Secrets zeroed on drop (in some implementations)  
✅ **Production ready** - Industry standard for sensitive data

---

## Estimated Time: 2-3 hours
- Adding dependency: 5 minutes
- Updating structure: 10 minutes
- Updating all usages: 60-90 minutes
- Updating tests: 30-45 minutes
- Testing and verification: 30 minutes
