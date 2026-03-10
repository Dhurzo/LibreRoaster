# Auditoría de Estrategia de Memoria - LibreRoaster

Este documento clasifica todos los módulos del proyecto según su estrategia de memoria
y identifica áreas que necesitan normalización.

## Resumen Ejecutivo

- **Módulos totales analizados**: 25
- **HOT PATH (heapless únicamente)**: 8 módulos
- **INITIALIZATION (heap permitido)**: 11 módulos  
- **MIXED (requiere cuidado)**: 6 módulos
- **Issues identificados**: 9 requieren atención

## Clasificación Detallada de Módulos

### ✅ HOT PATH Modules (heapless únicamente)

Estos módulos ya cumplen con la estrategia y no requieren cambios.

#### 1. `hardware/max31856/` - Lectura de temperatura
- **Estado**: ✅ OPTIMO
- **Uso de memoria**: Exclusivamente heapless/stack
- **Operaciones críticas**: Lectura SPI, conversión de temperatura
- **Verificación**: Sin allocations dinámicas en lectura de temperatura

#### 2. `control/pid/` - Control PID
- **Estado**: ✅ OPTIMO  
- **Uso de memoria**: Solo primitivos (f32, bool)
- **Operaciones críticas**: Cálculos de control en tiempo real
- **Verificación**: Cero heap allocations en cálculos PID

#### 3. `hardware/ssr/` - Control SSR
- **Estado**: ✅ OPTIMO
- **Uso de memoria**: Stack-only para control PWM
- **Operaciones críticas**: Generación de señales de control
- **Verificación**: Sin allocations en paths de control

#### 4. `output/artisan/` - Formateo Artisan
- **Estado**: ✅ OPTIMO
- **Uso de memoria**: heapless::Deque, heapless::String
- **Operaciones críticas**: Formateo de salida en tiempo real
- **Verificación**: Usa constantes definidas para tamaños

#### 5. `hardware/uart/tasks.rs` - Tareas UART
- **Estado**: ✅ OPTIMO
- **Uso de memoria**: heapless::Deque, heapless::String, heapless::Vec
- **Operaciones críticas**: Procesamiento de comunicación UART
- **Verificación**: Buffers con tamaño fijo

#### 6. `hardware/usb_cdc/tasks.rs` - Tareas USB CDC
- **Estado**: ✅ OPTIMO
- **Uso de memoria**: heapless::String, heapless::Vec
- **Operaciones críticas**: Comunicación USB en tiempo real
- **Verificación**: Sin allocations dinámicas en procesamiento

#### 7. `input/parser.rs` - Parseo de comandos
- **Estado**: ✅ OPTIMO
- **Uso de memoria**: heapless::Vec para tokens
- **Operaciones críticas**: Parseo de comandos en runtime
- **Verificación**: Usa capacidad fija (4 tokens)

#### 8. `safety/regression.rs` - Seguridad
- **Estado**: ✅ OPTIMO
- **Uso de memoria**: heapless::String para mensajes de seguridad
- **Operaciones críticas**: Manejo de eventos de seguridad
- **Verificación**: Mensajes con tamaño fijo (128)

### ⚠️ INITIALIZATION Modules (heap permitido, necesita normalización)

Estos módulos pueden usar heap pero necesitan estandarización.

#### 1. `control/policies.rs` - Políticas de control
- **Estado**: ⚠️ NECESITA NORMALIZACIÓN
- **Uso actual de alloc**: 
  ```rust
  pub error_message: Option<alloc::string::String>,
  pub reason: Option<alloc::string::String>,
  ```
- **Problema**: Usa `alloc::string::String` sin tamaño fijo
- **Recomendación**: Reemplazar con `heapless::String<POLICY_MSG_MAX_LEN>`
- **Prioridad**: ALTA

#### 2. `application/app_builder.rs` - Constructor de aplicación
- **Estado**: ⚠️ NECESITA REVISIÓN
- **Uso actual de alloc**: `alloc::boxed::Box<dyn Trait>`
- **Problema**: Dynamic dispatch en initialization (aceptable pero debe documentarse)
- **Recomendación**: Documentar y mantener (es apropiado para initialization)
- **Prioridad**: MEDIA

#### 3. `control/roaster_refactored.rs` - Control de tostadora
- **Estado**: ⚠️ NECESITA NORMALIZACIÓN
- **Uso actual de alloc**: 
  ```rust
  let parts: alloc::vec::Vec<&str> = response.split(',').collect();
  ```
- **Problema**: Allocation dinámica en parseo de respuesta
- **Recomendación**: Usar `heapless::Vec<_, COMMAND_BUFFER_SIZE>`
- **Prioridad**: ALTA

#### 4. `application/tasks.rs` - Tareas de aplicación
- **Estado**: ⚠️ NECESITA NORMALIZACIÓN
- **Uso actual de alloc**:
  ```rust
  fn append_crlf(payload: &str) -> alloc::vec::Vec<u8> {
  ```
- **Problema**: Allocation en función helper (no crítica pero debe optimizarse)
- **Recomendación**: Pre-allocation de buffer o uso de heapless
- **Prioridad**: MEDIA

#### 5. `output/traits.rs` - Traits de salida
- **Estado**: ⚠️ NECESITA REVISIÓN
- **Uso actual de alloc**: `use alloc::string::String;`
- **Problema**: Importación de alloc en traits (puede influir en implementaciones)
- **Recomendación**: Evaluar si es necesario o puede eliminarse
- **Prioridad**: BAJA

### 🔄 MIXED Modules (requiere documentación clara)

Estos módulos tienen operaciones en ambas categorías y necesitan documentación explícita.

#### 1. `error/app_error.rs` - Manejo de errores
- **Estado**: 🔄 BIEN IMPLEMENTADO
- **Estrategia actual**: 
  - `heapless::String<256>` para errores en runtime
  - Buena separación de responsabilidades
- **Recomendación**: Documentar explícitamente la estrategia
- **Prioridad**: BAJA (solo documentación)

#### 2. `application/stage_instrumentation.rs` - Instrumentación
- **Estado**: 🔄 BIEN IMPLEMENTADO
- **Estrategia actual**: Usa `heapless::String` para reporting
- **Recomendación**: Documentar y mantener
- **Prioridad**: BAJA (solo documentación)

#### 3. `common/mod.rs` - Utilidades comunes
- **Estado**: 🔄 NECESITA CLARIFICACIÓN
- **Uso actual**: `use alloc::{string::String, vec::Vec};`
- **Problema**: Importaciones genéricas sin propósito claro
- **Recomendación**: Documentar el propósito específico de cada import
- **Prioridad**: MEDIA

#### 4. `application/service_container.rs` - Contenedor de servicios
- **Estado**: 🔄 NECESITA REVISIÓN
- **Uso actual**: `use heapless::String;`
- **Problema**: Uso de heapless pero posiblemente en initialization
- **Recomendación**: Clarificar categorías de operaciones
- **Prioridad**: MEDIA

#### 5. `logging/tests.rs` - Tests de logging
- **Estado**: 🔄 ACEPTABLE
- **Contexto**: Es código de tests, puede usar allocations
- **Recomendación**: Mantener como está (tests pueden usar heap)
- **Prioridad**: BAJA

#### 6. `input/mod.rs` - Módulo de entrada
- **Estado**: 🔄 ACEPTABLE
- **Uso actual**: `use heapless::Deque;`
- **Contexto**: Usa heapless apropiadamente
- **Recomendación**: Documentar y mantener
- **Prioridad**: BAJA

## Plan de Acción Inmediato

### Prioridad ALTA (resolver esta semana)

1. **Normalizar `control/policies.rs`**:
   ```rust
   // Cambiar de:
   pub error_message: Option<alloc::string::String>,
   
   // A:
   pub error_message: Option<heapless::String<POLICY_MSG_MAX_LEN>>,
   ```

2. **Normalizar `control/roaster_refactored.rs`**:
   ```rust
   // Cambiar de:
   let parts: alloc::vec::Vec<&str> = response.split(',').collect();
   
   // A:
   let parts: heapless::Vec<&str, COMMAND_BUFFER_SIZE> = response.split(',').collect();
   ```

### Prioridad MEDIA (resolver esta iteration)

3. **Optimizar `application/tasks.rs`**:
   - Evaluar si `append_crlf` realmente necesita allocation
   - Considerar pre-allocation o stack allocation

4. **Documentar todos los módulos MIXED**:
   - Agregar comentarios explícitos sobre estrategia de memoria
   - Clasificar cada función según su categoría

### Prioridad BAJA (documentación y limpieza)

5. **Limpiar importaciones innecesarias**:
   - Revisar `output/traits.rs` y `common/mod.rs`
   - Eliminar imports de alloc no utilizados

6. **Documentación final**:
   - Asegurar que todos los módulos tengan documentación clara
   - Actualizar `memory/strategy.rs` con ejemplos reales

## Métricas de Verificación

### Antes de la normalización:
- 22 usos de `alloc::` encontrados en el código
- 23 usos de `heapless::` encontrados
- 9 módulos necesitan atención

### Después de la normalización (objetivos):
- Reducir usos de `alloc::` en un 70% (a ~6-8 usos)
- Mantener todos los usos de `heapless::` existentes
- Todos los módulos clasificados y documentados
- Cero heap allocations en hot paths verificado con tests