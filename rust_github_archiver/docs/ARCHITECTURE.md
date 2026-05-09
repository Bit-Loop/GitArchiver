# System Architecture Documentation

## Overview

GitHub Archiver is a production-grade Rust application for archiving GitHub events, detecting secrets, and providing real-time monitoring capabilities.

## Architecture Diagrams

### High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Internet / Users                            │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           │ HTTPS
                           │
┌──────────────────────────▼──────────────────────────────────────────┐
│                     Load Balancer / Ingress                          │
│                    (Kubernetes Ingress / nginx)                      │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                  ┌────────┴────────┐
                  │                 │
┌─────────────────▼────┐   ┌────────▼──────────────┐
│  GitHub Archiver     │   │  Monitoring Stack     │
│  Application         │   │                       │
│  (3-10 replicas)     │   │  - Prometheus         │
│                      │   │  - Grafana            │
│  - REST API          │   │  - Alertmanager       │
│  - Secret Scanner    │   └───────────────────────┘
│  - Event Processor   │
│  - Real-time Monitor │
└──────────┬───────────┘
           │
           │
┌──────────▼───────────────────────────────────────────────────────┐
│                     PostgreSQL Database                           │
│                     (StatefulSet with PV)                         │
│                                                                   │
│  - Events storage                                                 │
│  - Secrets database                                               │
│  - User management                                                │
│  - API keys                                                       │
└───────────────────────────────────────────────────────────────────┘
```

### Application Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                      GitHub Archiver Application                    │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────┐     │
│  │                      API Layer (Axum)                     │     │
│  │                                                           │     │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │     │
│  │  │   Auth      │  │   Events    │  │   Secrets   │     │     │
│  │  │  Endpoints  │  │  Endpoints  │  │  Endpoints  │     │     │
│  │  └─────────────┘  └─────────────┘  └─────────────┘     │     │
│  │                                                           │     │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │     │
│  │  │   Admin     │  │  Monitoring │  │   Health    │     │     │
│  │  │  Endpoints  │  │  Endpoints  │  │   Checks    │     │     │
│  │  └─────────────┘  └─────────────┘  └─────────────┘     │     │
│  └──────────────────────────────────────────────────────────┘     │
│                              │                                     │
│  ┌──────────────────────────▼──────────────────────────────┐     │
│  │                    Middleware Layer                      │     │
│  │                                                           │     │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐  │     │
│  │  │   Rate   │  │ Security │  │   CORS   │  │  Auth  │  │     │
│  │  │ Limiting │  │ Headers  │  │          │  │        │  │     │
│  │  └──────────┘  └──────────┘  └──────────┘  └────────┘  │     │
│  │                                                           │     │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐  │     │
│  │  │ Request  │  │  Logging │  │ Metrics  │  │ Timeout│  │     │
│  │  │   Size   │  │          │  │          │  │        │  │     │
│  │  └──────────┘  └──────────┘  └──────────┘  └────────┘  │     │
│  └──────────────────────────────────────────────────────────┘     │
│                              │                                     │
│  ┌──────────────────────────▼──────────────────────────────┐     │
│  │                     Core Services                        │     │
│  │                                                           │     │
│  │  ┌───────────────┐  ┌───────────────┐  ┌────────────┐  │     │
│  │  │   Database    │  │  Secret       │  │  GitHub    │  │     │
│  │  │   Manager     │  │  Scanner      │  │  Client    │  │     │
│  │  └───────────────┘  └───────────────┘  └────────────┘  │     │
│  │                                                           │     │
│  │  ┌───────────────┐  ┌───────────────┐  ┌────────────┐  │     │
│  │  │   Event       │  │  Webhook      │  │  Circuit   │  │     │
│  │  │   Processor   │  │  Manager      │  │  Breaker   │  │     │
│  │  └───────────────┘  └───────────────┘  └────────────┘  │     │
│  └──────────────────────────────────────────────────────────┘     │
│                              │                                     │
│  ┌──────────────────────────▼──────────────────────────────┐     │
│  │                   Infrastructure                         │     │
│  │                                                           │     │
│  │  ┌───────────────┐  ┌───────────────┐  ┌────────────┐  │     │
│  │  │  Health       │  │   Graceful    │  │  Metrics   │  │     │
│  │  │  Checks       │  │   Shutdown    │  │  Export    │  │     │
│  │  └───────────────┘  └───────────────┘  └────────────┘  │     │
│  └──────────────────────────────────────────────────────────┘     │
└────────────────────────────────────────────────────────────────────┘
```

### Data Flow Diagram

```
┌─────────────┐
│   GitHub    │
│     API     │
└──────┬──────┘
       │
       │ Events
       │
┌──────▼────────────────────────────────────────────────────┐
│                   GitHub Archiver                          │
│                                                            │
│  1. Event Ingestion                                        │
│     └─> Rate Limiting ──> Validation ──> Storage          │
│                                                            │
│  2. Secret Scanning                                        │
│     └─> Pattern Matching ──> Validation ──> Alert         │
│                                                            │
│  3. Real-time Processing                                   │
│     └─> Stream Processing ──> Transformation ──> Output   │
└────────────────┬───────────────────────────────────────────┘
                 │
        ┌────────┴─────────┐
        │                  │
┌───────▼────────┐  ┌──────▼──────┐
│   PostgreSQL   │  │   Webhooks  │
│   Database     │  │   (Slack/   │
│                │  │   Discord)  │
└────────────────┘  └─────────────┘
```

### Deployment Architecture (Kubernetes)

```
┌─────────────────────────────────────────────────────────────┐
│                  Kubernetes Cluster                          │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │           Namespace: github-archiver               │    │
│  │                                                     │    │
│  │  ┌─────────────────────────────────────────────┐  │    │
│  │  │          Ingress (nginx)                    │  │    │
│  │  │  - TLS termination                          │  │    │
│  │  │  - Load balancing                           │  │    │
│  │  └────────────┬────────────────────────────────┘  │    │
│  │               │                                    │    │
│  │  ┌────────────▼────────────────────────────────┐  │    │
│  │  │    Service: github-archiver (LoadBalancer)  │  │    │
│  │  │    - Port: 80 → 8081                        │  │    │
│  │  └────────────┬────────────────────────────────┘  │    │
│  │               │                                    │    │
│  │  ┌────────────▼────────────────────────────────┐  │    │
│  │  │    HorizontalPodAutoscaler                  │  │    │
│  │  │    - Min replicas: 3                        │  │    │
│  │  │    - Max replicas: 10                       │  │    │
│  │  │    - CPU target: 70%                        │  │    │
│  │  │    - Memory target: 80%                     │  │    │
│  │  └────────────┬────────────────────────────────┘  │    │
│  │               │                                    │    │
│  │  ┌────────────▼────────────────────────────────┐  │    │
│  │  │    Deployment: github-archiver              │  │    │
│  │  │                                             │  │    │
│  │  │  ┌────────┐  ┌────────┐  ┌────────┐       │  │    │
│  │  │  │  Pod 1 │  │  Pod 2 │  │  Pod 3 │       │  │    │
│  │  │  │        │  │        │  │        │       │  │    │
│  │  │  │ App    │  │ App    │  │ App    │       │  │    │
│  │  │  │ 512Mi- │  │ 512Mi- │  │ 512Mi- │       │  │    │
│  │  │  │ 2Gi    │  │ 2Gi    │  │ 2Gi    │       │  │    │
│  │  │  └────────┘  └────────┘  └────────┘       │  │    │
│  │  └─────────────────────────────────────────────┘  │    │
│  │                      │                             │    │
│  │  ┌───────────────────▼─────────────────────────┐  │    │
│  │  │    StatefulSet: postgres                    │  │    │
│  │  │                                             │  │    │
│  │  │  ┌────────┐                                 │  │    │
│  │  │  │postgres│                                 │  │    │
│  │  │  │  -0    │                                 │  │    │
│  │  │  │        │                                 │  │    │
│  │  │  │ 256Mi- │                                 │  │    │
│  │  │  │ 1Gi    │                                 │  │    │
│  │  │  └────┬───┘                                 │  │    │
│  │  │       │                                     │  │    │
│  │  │  ┌────▼────────────────┐                   │  │    │
│  │  │  │ PersistentVolume    │                   │  │    │
│  │  │  │ 10Gi                │                   │  │    │
│  │  │  └─────────────────────┘                   │  │    │
│  │  └─────────────────────────────────────────────┘  │    │
│  │                                                     │    │
│  │  ┌─────────────────────────────────────────────┐  │    │
│  │  │    ConfigMap: github-archiver-config        │  │    │
│  │  │    - SERVER_PORT=8081                       │  │    │
│  │  │    - RUST_LOG=info                          │  │    │
│  │  └─────────────────────────────────────────────┘  │    │
│  │                                                     │    │
│  │  ┌─────────────────────────────────────────────┐  │    │
│  │  │    Secret: github-archiver-secrets          │  │    │
│  │  │    - POSTGRES_PASSWORD                      │  │    │
│  │  │    - JWT_SECRET                             │  │    │
│  │  │    - GITHUB_TOKEN                           │  │    │
│  │  └─────────────────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Security Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Security Layers                         │
│                                                              │
│  Layer 1: Network Security                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  - TLS/SSL encryption (HTTPS)                        │  │
│  │  - Ingress with certificate management (cert-manager)│  │
│  │  - Network policies (allow-list)                     │  │
│  └──────────────────────────────────────────────────────┘  │
│                         │                                   │
│  Layer 2: Application Security                              │
│  ┌──────────────────────▼──────────────────────────────┐  │
│  │  - Rate limiting (token bucket per IP)              │  │
│  │  - CORS (origin validation)                         │  │
│  │  - Security headers (HSTS, CSP, X-Frame-Options)    │  │
│  │  - Request size limits (10MB body, 8KB headers)     │  │
│  │  - Request timeout (30 seconds)                     │  │
│  └──────────────────────────────────────────────────────┘  │
│                         │                                   │
│  Layer 3: Authentication & Authorization                    │
│  ┌──────────────────────▼──────────────────────────────┐  │
│  │  - JWT tokens (HS256, 1-hour expiry)                │  │
│  │  - API key validation                               │  │
│  │  - Role-based access control (RBAC)                 │  │
│  │  - Session management                               │  │
│  └──────────────────────────────────────────────────────┘  │
│                         │                                   │
│  Layer 4: Data Security                                     │
│  ┌──────────────────────▼──────────────────────────────┐  │
│  │  - Database encryption at rest                      │  │
│  │  - Encrypted backups (S3 with encryption)           │  │
│  │  - Secret management (Kubernetes Secrets)           │  │
│  │  - Audit logging                                    │  │
│  └──────────────────────────────────────────────────────┘  │
│                         │                                   │
│  Layer 5: Infrastructure Security                           │
│  ┌──────────────────────▼──────────────────────────────┐  │
│  │  - Pod security policies                            │  │
│  │  - Resource limits (CPU, memory)                    │  │
│  │  - Container security scanning (CI/CD)              │  │
│  │  - Secrets rotation                                 │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Monitoring Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Monitoring Stack                          │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              Application Metrics                      │  │
│  │                                                       │  │
│  │  /metrics endpoint (Prometheus format)               │  │
│  │                                                       │  │
│  │  - HTTP request rate, duration, status               │  │
│  │  - Active connections                                │  │
│  │  - Circuit breaker state                             │  │
│  │  - Rate limiter statistics                           │  │
│  │  - Events processed, secrets detected                │  │
│  └─────────────────┬─────────────────────────────────────┘  │
│                    │                                         │
│  ┌─────────────────▼─────────────────────────────────────┐  │
│  │            Prometheus (Metrics Collection)            │  │
│  │                                                       │  │
│  │  - Scrapes metrics every 10-15 seconds               │  │
│  │  - Stores time-series data                           │  │
│  │  - Evaluates alert rules                             │  │
│  └─────────────────┬─────────────────────────────────────┘  │
│                    │                                         │
│         ┌──────────┴──────────┐                             │
│         │                     │                             │
│  ┌──────▼─────────┐   ┌───────▼────────┐                   │
│  │  Alertmanager  │   │    Grafana     │                   │
│  │                │   │                │                   │
│  │  - Routes      │   │  - Dashboards  │                   │
│  │  - Dedup       │   │  - Queries     │                   │
│  │  - Notifies    │   │  - Alerts UI   │                   │
│  └────────┬───────┘   └────────────────┘                   │
│           │                                                 │
│  ┌────────▼─────────────────────────────────────────────┐  │
│  │           Notification Channels                      │  │
│  │                                                       │  │
│  │  - Slack webhooks                                    │  │
│  │  - Discord webhooks                                  │  │
│  │  - Email (SMTP)                                      │  │
│  │  - PagerDuty                                         │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Component Details

### API Server
- **Framework**: Axum (Rust async web framework)
- **Port**: 8081 (configurable)
- **Endpoints**: REST API for events, secrets, admin
- **Auth**: JWT-based authentication
- **Middleware**: Rate limiting, security headers, CORS, logging

### Database
- **Type**: PostgreSQL 15
- **Storage**: Persistent volume (10Gi default)
- **Connection Pool**: 10-150 connections (configurable)
- **Backup**: Daily automated backups to S3
- **Schema**: Events, secrets, users, API keys, webhooks

### Secret Scanner
- **Engine**: Multi-pattern regex + validation
- **Patterns**: AWS keys, GitHub tokens, API keys, etc.
- **Validation**: Live credential checking
- **Performance**: Parallel processing (4-8 threads)

### Monitoring
- **Metrics**: Prometheus format on /metrics
- **Dashboards**: Grafana (main + database)
- **Alerts**: 20+ production alerts
- **Health**: 3 endpoints (live, ready, detailed)

### Security
- **Rate Limiting**: 1000 req/min (authenticated), 100 req/min (public)
- **Headers**: HSTS, CSP, X-Frame-Options, X-Content-Type-Options
- **CORS**: Configurable origins
- **Timeouts**: 30-second request timeout
- **Size Limits**: 10MB body, 8KB headers

### High Availability
- **Replicas**: 3-10 (auto-scaling based on CPU/memory)
- **Health Checks**: Liveness (always), readiness (DB check)
- **Graceful Shutdown**: 30-second grace period
- **Circuit Breakers**: Prevent cascading failures
- **Retry Logic**: Exponential backoff for external calls

## Technology Stack

### Core
- **Language**: Rust 1.75+
- **Async Runtime**: Tokio
- **Web Framework**: Axum
- **Database**: PostgreSQL 15 with SQLx

### Infrastructure
- **Container**: Docker multi-stage builds
- **Orchestration**: Kubernetes 1.27+
- **Monitoring**: Prometheus + Grafana
- **CI/CD**: GitHub Actions

### Libraries
- **Authentication**: JWT (jsonwebtoken)
- **Metrics**: prometheus crate
- **Logging**: tracing + tracing-subscriber
- **HTTP Client**: reqwest
- **Configuration**: config + dotenv

## Scalability

### Horizontal Scaling
- **Auto-scaling**: 3-10 replicas based on CPU (70%) and memory (80%)
- **Load Balancing**: Kubernetes service with round-robin
- **Stateless**: Application pods are stateless (state in DB)

### Vertical Scaling
- **Resources**: 512Mi-2Gi RAM, 500m-2000m CPU per pod
- **Database**: Can scale vertically with larger instance
- **Connection Pool**: Adjustable (10-150 connections)

### Performance Targets
- **API Response Time**: <100ms p95
- **Throughput**: >10,000 events/second
- **Uptime**: 99.9% availability
- **Recovery Time**: <1 hour (RTO)
- **Recovery Point**: <24 hours (RPO)

## Security Considerations

### Data Protection
- Database encryption at rest
- TLS/SSL for all connections
- Encrypted backups
- Secret rotation

### Access Control
- JWT token authentication
- Role-based access control (RBAC)
- API key management
- Audit logging

### Network Security
- Network policies (allow-list)
- Ingress with TLS termination
- Rate limiting per IP
- DDoS protection (infrastructure level)

### Compliance
- GDPR considerations for data retention
- Audit trail for admin actions
- Secure secret handling
- Incident response procedures

## Disaster Recovery

### Backup Strategy
- **Frequency**: Daily at 02:00 UTC
- **Retention**: 30 days local, 90 days S3
- **Type**: Full PostgreSQL dump (compressed)
- **Verification**: Checksum validation

### Recovery Procedures
1. Restore from latest backup
2. Verify data integrity
3. Update DNS if needed
4. Run smoke tests
5. Monitor for issues

### RTO/RPO
- **RTO**: 1 hour (Recovery Time Objective)
- **RPO**: 24 hours (Recovery Point Objective)

---

**Last Updated**: 2025-01-15  
**Version**: 1.0.0  
**Author**: GitHub Archiver Team
