# 🔐 Security Advisory - Critical Issues Found

## ⚠️ CRITICAL SECURITY ISSUES

### 1. **Weak Default Admin Password** - SEVERITY: HIGH
**Location**: `src/auth/users.rs:30`
**Status**: Fixed; startup now rejects missing, weak, and documented/default admin passwords.

**Issue**:
```rust
let admin_password = std::env::var("ADMIN_PASSWORD")
    .unwrap_or_else(|_| "<unsafe-default-password>".to_string());
```

**Risk**:
- A documented/default password was publicly visible in source code
- Attackers can gain admin access if `.env` is not properly configured
- Default credentials are commonly exploited

**Immediate Action Required**:
```bash
# Generate a strong password
openssl rand -base64 32

# Add to .env file
echo "ADMIN_PASSWORD=<your_strong_password_here>" >> .env
```

**Recommended Fix**:
```rust
let admin_password = std::env::var("ADMIN_PASSWORD")
    .expect("ADMIN_PASSWORD must be set in environment variables. NEVER use default passwords in production!");
```

---

### 2. **Sensitive .env File in Repository** - SEVERITY: CRITICAL
**Location**: `rust_github_archiver/.env`

**Issue**:
- Actual `.env` file detected in repository
- May contain real credentials, API keys, database passwords
- Could be committed to git history

**Immediate Action Required**:
```bash
# Remove .env from git tracking
git rm --cached .env

# Ensure it's in .gitignore (now added)
echo ".env" >> .gitignore

# Commit the changes
git commit -m "security: Remove .env file from tracking"

# Verify it's gone from new commits
git status
```

**Check Git History**:
```bash
# Search git history for sensitive data
git log --all --full-history -- .env

# If found in history, consider using git-filter-repo or BFG Repo-Cleaner
# to remove sensitive data from git history
```

---

### 3. **Missing .gitignore** - SEVERITY: MEDIUM
**Location**: `rust_github_archiver/.gitignore`

**Issue**:
- No `.gitignore` file found
- Sensitive files, logs, credentials could be accidentally committed

**Action Taken**:
✅ Created comprehensive `.gitignore` with:
- `.env` and environment files
- Database files
- Log files
- API keys and secrets
- Build artifacts
- IDE files
- Temporary data

---

### 4. **Port Configuration Bug** - SEVERITY: LOW
**Location**: `src/bin/web_server.rs:22,25`

**Issue**:
```rust
config.web.port = web_port.parse().unwrap_or(8090);      // Wrong default
config.database.port = db_port.parse().unwrap_or(8091);  // Wrong field
```

**Problems**:
- Default web port should be 8081 (as per `.env.example`)
- Setting `database.port` when it should be database connection port
- Inconsistent with documented defaults

**Recommended Fix**:
```rust
if let Ok(web_port) = env::var("WEB_PORT").or_else(|_| env::var("API_PORT")) {
    config.web.port = web_port.parse().unwrap_or(8081); // Correct default
}
if let Ok(db_port) = env::var("DB_PORT") {
    config.database.port = db_port.parse().unwrap_or(5432); // PostgreSQL default
}
```

---

## 🛡️ Security Checklist

### Immediate Actions (DO NOW):
- [ ] Change `ADMIN_PASSWORD` in `.env` to a strong password
- [ ] Remove `.env` from git tracking
- [ ] Generate new `JWT_SECRET` (32+ bytes)
- [ ] Verify no secrets in git history
- [ ] Review database credentials

### Configuration Security:
- [ ] Use environment variables for ALL secrets
- [ ] Never commit `.env` files
- [ ] Use `.env.example` as template only
- [ ] Rotate all API keys and tokens
- [ ] Use strong passwords (20+ characters, random)

### Deployment Security:
- [ ] Use HTTPS in production
- [ ] Enable JWT token expiration
- [ ] Implement rate limiting
- [ ] Add fail2ban or similar
- [ ] Monitor for unauthorized access attempts

### Code Security:
- [ ] Remove all default passwords from source code
- [ ] Use `.expect()` instead of `.unwrap_or()` for critical secrets
- [ ] Add security headers to web server
- [ ] Implement CSRF protection
- [ ] Validate all user inputs

---

## 🔧 Recommended Security Improvements

### 1. Environment Variable Validation
Add startup validation:

```rust
pub fn validate_security_config() -> Result<()> {
    // Ensure critical secrets are set
    env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD must be set");
    
    // Warn about weak passwords
    let admin_pw = env::var("ADMIN_PASSWORD")?;
    if admin_pw.len() < 12 {
        warn!("⚠️  ADMIN_PASSWORD is too short. Use 20+ characters!");
    }
    if is_documented_or_common_password(&admin_pw) {
        return Err(anyhow!("❌ NEVER use default passwords in production!"));
    }
    
    Ok(())
}
```

### 2. JWT Secret Generation
```bash
# Generate secure JWT secret
openssl rand -hex 32

# Or in Rust
use rand::Rng;
let jwt_secret: String = rand::thread_rng()
    .sample_iter(&rand::distributions::Alphanumeric)
    .take(64)
    .map(char::from)
    .collect();
```

### 3. Password Hashing
Ensure bcrypt/argon2 is used (already implemented ✓):
```rust
use argon2::{Argon2, PasswordHasher};
// Good - already using secure password hashing
```

### 4. Security Headers
Add to web server:
```rust
use tower_http::set_header::SetResponseHeaderLayer;

.layer(SetResponseHeaderLayer::overriding(
    header::X_FRAME_OPTIONS,
    HeaderValue::from_static("DENY"),
))
.layer(SetResponseHeaderLayer::overriding(
    header::X_CONTENT_TYPE_OPTIONS,
    HeaderValue::from_static("nosniff"),
))
.layer(SetResponseHeaderLayer::overriding(
    header::STRICT_TRANSPORT_SECURITY,
    HeaderValue::from_static("max-age=31536000; includeSubDomains"),
))
```

---

## 📋 Environment Variables Security Guide

### Critical Variables (MUST be unique):
```bash
# Generate with: openssl rand -hex 32
JWT_SECRET=<64_character_random_hex>

# Generate with: openssl rand -base64 32  
ADMIN_PASSWORD=<strong_random_password>

# Use real GitHub token (not placeholder)
GITHUB_TOKEN=ghp_<your_real_token_here>
```

### Database Security:
```bash
# Use strong database password
DB_PASSWORD=<complex_password_20+_chars>

# Limit database access
DB_HOST=localhost  # Don't expose externally unless needed
```

### Production Deployment:
```bash
# Enable production mode
RUST_ENV=production

# Disable debug logging in production
RUST_LOG=info,github_archiver=warn

# Use secure session settings
SESSION_TIMEOUT_MINUTES=30  # Shorter timeout
```

---

## 🚨 Vulnerability Summary

| Issue | Severity | Status | Action Required |
|-------|----------|--------|-----------------|
| Default admin password | **HIGH** | 🔴 Open | Change immediately |
| `.env` in repository | **CRITICAL** | 🔴 Open | Remove from git |
| Missing `.gitignore` | **MEDIUM** | ✅ Fixed | Review other files |
| Port config bug | **LOW** | 🟡 Open | Fix defaults |

---

## 📞 Incident Response

If credentials have been exposed:
1. **Immediately** rotate all secrets (passwords, API keys, JWT secrets)
2. **Revoke** any exposed GitHub tokens
3. **Reset** admin password
4. **Review** database for unauthorized access
5. **Audit** git history for sensitive data
6. **Consider** re-encrypting sensitive database fields

---

## ✅ Post-Fix Verification

After fixing security issues:
```bash
# Verify .env is not tracked
git status | grep .env  # Should return nothing

# Verify .gitignore is working
git check-ignore .env   # Should output: .env

# Verify no secrets in current code
grep -r "unsafe-default-password" src/  # Should find no production fallback

# Verify strong passwords
echo $ADMIN_PASSWORD | wc -c  # Should be 20+ characters
```

---

## 📚 References

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Security Book](https://rust-lang.github.io/unsafe-code-guidelines/)
- [CWE-798: Use of Hard-coded Credentials](https://cwe.mitre.org/data/definitions/798.html)
- [git-secrets](https://github.com/awslabs/git-secrets) - Prevent committing secrets

---

**Last Updated**: October 4, 2025
**Severity Level**: 🔴 CRITICAL - Immediate action required
