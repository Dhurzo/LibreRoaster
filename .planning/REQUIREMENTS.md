# Requirements: LibreRoaster

## Milestone v1.7: Non-Blocking USB Logging

| ID | Description | Phase | Status |
|----|-------------|-------|--------|
| LOG-01 | Capturar y loguear cada comando ASCII recibido por USB CDC. | 23 | Pending |
| LOG-02 | Capturar y loguear cada respuesta ASCII enviada por USB CDC. | 23 | Pending |
| LOG-03 | Incluir el prefijo [USB] en todos los logs de este canal. | 22 | Pending |
| LOG-06 | No bloqueante usando defmt + bbqueue para proteger el PID. | 21 | Pending |

## v1.7 Goals

- Ensure PID control loop (100ms) is never blocked by logging.
- Provide full visibility into Artisan <-> Roaster communication over USB.
- Establish a scalable, high-performance logging foundation for future features.

## Coverage Summary
- **Foundation**: Phase 21 (LOG-06)
- **Metadata/Transport**: Phase 22 (LOG-03)
- **Monitoring**: Phase 23 (LOG-01, LOG-02)

## Previous Milestones
Archived in `.planning/milestones/`.
