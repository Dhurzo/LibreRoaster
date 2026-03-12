# Resumen de la Arquitectura de LibreRoaster

Este documento proporciona una visión general de la arquitectura del software para el proyecto LibreRoaster, un sistema de control de tostador de café basado en Rust y diseñado para hardware ESP32-C3.

**Estado de Integración Hardware:** ✅ **REAL** - El sistema tiene integración hardware completa con control real de fans, SSRs y lectura de temperaturas. Ver `HARDWARE.md` para detalles específicos.

**Estado de Integración ARTISAN+:** ✅ **COMPLETA** - Comunicación bidireccional funcional con control real del tostador desde el software Artisan.

## 1. Resumen de la Arquitectura

La aplicación sigue un diseño modular y basado en tareas asíncronas, aprovechando el framework `embassy` para la programación concurrente en sistemas embebidos. La arquitectura se puede dividir en las siguientes capas principales:

- **Capa de Aplicación (`application`):** Orquesta el ciclo de vida de la aplicación. Utiliza un patrón `Builder` (`AppBuilder`) para inicializar y configurar los diferentes módulos y un `ServiceContainer` (patrón Singleton) para proporcionar acceso seguro a los servicios compartidos entre las tareas asíncronas.
- **Capa de Control (`control`):** Contiene la lógica de negocio principal del tostador. Gestiona el estado del proceso de tueste, interpreta los comandos y utiliza controladores (como PID) para mantener la temperatura deseada.
- **Capa de Hardware (`hardware`):** Abstrae la comunicación con los periféricos físicos. Proporciona drivers para componentes como el ventilador (Fan), el relé de estado sólido (SSR), los sensores de temperatura (MAX31856) y la comunicación UART.
- **Capa de Entrada/Salida (`input`/`output`):** Gestiona la comunicación con el exterior, principalmente a través de una interfaz serie compatible con el software [Artisan](https://artisan-scope.org/). El módulo `input` parsea los comandos entrantes y el módulo `output` formatea los datos de estado y telemetría para su envío.
- **Capa de Servidor (`server`):** Expone una interfaz HTTP para control o monitoreo alternativo.

El sistema está diseñado para ser robusto y seguro en un entorno concurrente, utilizando primitivas de sincronización (`Mutex` de `critical-section`) para evitar condiciones de carrera al acceder a recursos compartidos.

## 2. Integración ARTISAN+ (Bidireccional Real)

Esta sección documenta la integración completa con el software Artisan para control de la tostadora.

### 2.1 Comandos Soportados (PC → Tostadora)

| Comando | Formato | Acción |
|---------|---------|--------|
| READ | `READ` | Retorna estado: `ET,BT,Power,Fan` |
| START | `START` | Inicia tueste y streaming continuo |
| OT1 x | `OT1 <0-100>` | Control manual del calentador (%) |
| IO3 x | `IO3 <0-100>` | Control manual del ventilador (%) |
| STOP | `STOP` | Parada de emergencia |

**Referencia:** `src/input/parser.rs:10-46`

### 2.2 Datos de Salida (Tostadora → PC)

**Formato CSV:** `time,ET,BT,ROR,Gas`

| Campo | Descripción | Unidades |
|-------|-------------|----------|
| time | Tiempo desde inicio | Segundos (0.00) |
| ET | Temperatura ambiental | °C (1 decimal) |
| BT | Temperatura de granos | °C (1 decimal) |
| ROR | Rate of Rise | °C/s (2 decimales) |
| Gas | Potencia SSR | % (1 decimal) |

**Frecuencia:** 10 Hz (cada 100ms)
**Referencia:** `src/output/artisan.rs:76-78`

### 2.3 Configuración UART

| Parámetro | Valor |
|-----------|-------|
| Baud rate | 115200 |
| TX (T→PC) | GPIO20 |
| RX (P→T) | GPIO21 |
| Formato | 8N1 |

**Referencia:** `src/hardware/uart/driver.rs:51`, `src/config/constants.rs:14-15`

### 2.4 Flujo de Datos ARTISAN+

```
┌─────────────────────────────────────────────────────────────────┐
│                    ARQUITECTURA ARTISAN+                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────┐    GPIO21    ┌──────────────┐                   │
│   │  Artisan │  ◄────────── │ UART RX      │                   │
│   │  (PC)    │              │ (driver.rs)  │                   │
│   └────┬─────┘              └──────┬───────┘                   │
│        │                            │                           │
│        │ TX (Comandos)              │ Parser                    │
│        ▼                            ▼                           │
│   ┌──────────┐    GPIO20    ┌──────────────┐    ┌──────────┐  │
│   │  Artisan │  ──────────► │ UART TX      │───►│  Parser  │  │
│   │  (PC)    │              │ (driver.rs)  │    │(parser.rs)  │
│   └──────────┘              └──────┬───────┘    └─────┬──────┘
│                                     │                  │
│                                     │                  ▼
│                                     │         ┌──────────────┐
│                                     │         │ ArtisanInput │
│                                     │         │(input/mod.rs)│
│                                     │         └──────┬───────┘
│                                     │                │
│                                     │                │ ArtisanCommand
│                                     │                ▼
│                                     │         ┌──────────────┐
│                                     │         │ RoasterControl│
│                                     │         │(roaster_refactored)│
│                                     │         └──────┬───────┘
│                                     │                │
│   ┌──────────┐              ┌───────▼───────┐       │
│   │  Artisan │ ◄────────────┤ ArtisanFormatter│◄──────┤
│   │  (PC)    │               │(output/artisan.rs)│    │
│   └──────────┘               └────────────────┘       │
│                                                         │
└─────────────────────────────────────────────────────────────────┘
```

### 2.5 Tareas Asíncronas ARTISAN+

| Tarea | Frecuencia | Descripción |
|-------|------------|-------------|
| `control_loop_task` | 10Hz | Lee sensores, procesa comandos, formatea salida |
| `artisan_output_task` | Event-driven | Envía datos formateados por UART |
| `uart_reader_task` | ~100Hz | Lee comandos entrantes |
| `uart_writer_task` | Event-driven | Escribe respuesta a UART |

**Referencia:** `src/application/tasks.rs:8-112`

### 2.6 Canal de Comunicación

El sistema usa canales asíncronos para comunicar comandos:

```rust
// src/application/service_container.rs
static ARTISAN_CMD_CHANNEL: Channel<
    CriticalSectionRawMutex,
    ArtisanCommand,
    ARTISAN_CMD_CHANNEL_SIZE,  // 8 slots
> = Channel::new();
```

### 2.7 Procesamiento de Comandos

```rust
// src/control/roaster_refactored.rs:223-272
pub fn process_artisan_command(&mut self, command: ArtisanCommand) {
    match command {
        ArtisanCommand::StartRoast => {
            self.status.artisan_control = true;
            // Iniciar control desde Artisan
        }
        ArtisanCommand::SetHeater(value) => {
            self.artisan_handler.set_manual_heater(value);
            // Override PID con valor manual
        }
        ArtisanCommand::SetFan(value) => {
            self.artisan_handler.set_manual_fan(value);
            // Control directo del ventilador
        }
        ArtisanCommand::EmergencyStop => {
            self.safety_handler.trigger_emergency("Artisan+ emergency stop");
        }
        _ => {}
    }
}
```

## 3. Responsabilidades de Ficheros y Módulos

A continuación se detalla la función de los directorios y ficheros clave en `src/`:

- **`main.rs`**: Punto de entrada de la aplicación. Se encarga de la inicialización del hardware específico de la placa (periféricos del ESP32-C3), configura el `AppBuilder` con los recursos de hardware y arranca las tareas principales de la aplicación.

- **`application/`**:
  - `app_builder.rs`: Implementa el patrón `Builder` para construir el objeto `Application`. Permite una configuración fluida de los componentes del sistema (UART, SSR, sensores, etc.).
  - `service_container.rs`: Implementa un Singleton que actúa como contenedor de servicios. Almacena instancias compartidas del `RoasterControl`, `FanController`, etc., y proporciona acceso seguro a ellas desde cualquier tarea.
  - `tasks.rs`: Define las tareas asíncronas principales, como el bucle de control del tueste y el manejador de comandos de Artisan.

- **`control/`**:
  - `roaster_refactored.rs`: Lógica principal y máquina de estados del proceso de tueste (`RoasterControl`). Coordina los diferentes manejadores.
  - `handlers.rs`: Implementación de manejadores específicos para diferentes tipos de comandos (Temperatura, Seguridad, Sistema, Artisan).
  - `abstractions.rs`: Traits y definiciones comunes para el sistema de control, incluyendo la definición de `RoasterCommandHandler`.
  - `command_handler.rs`: Estructuras de comandos base.
  - `pid.rs`: Wrapper del controlador PID utilizado por el sistema de control. La implementación detallada se encuentra en `control/pid.rs`.

- **`hardware/`**:
  - `fan.rs`: Driver para controlar la velocidad del ventilador usando PWM.
  - `ssr.rs`: Driver para controlar el relé de estado sólido (SSR) que activa la resistencia de calor.
  - `max31856.rs`: Driver para comunicarse con los termopares MAX31856 a través de SPI.
  - `pid.rs`: Implementación del controlador PID específico para tostadoras de café. La implementación lógica se encuentra en `control/pid.rs`.
  - `shared_spi.rs`: Utilidades para manejar el acceso compartido al bus SPI.
  - `uart/`: Módulo de bajo nivel para la comunicación serie.

- **`config/`**:
  - `constants.rs`: Constantes globales de configuración (pines, límites de temperatura, ganancias PID, etc.).

- **`error/`**:
  - `app_error.rs`: Definiciones centralizadas de errores de la aplicación.

- **`input/`**:
  - `mod.rs`: Exporta `ArtisanInput` y `parse_artisan_command`
  - `parser.rs`: Parseo de comandos Artisan (READ, START, OT1, IO3, STOP)

- **`output/`**:
  - `manager.rs`: Gestor principal de salidas
  - `artisan.rs`: Formato CSV `time,ET,BT,ROR,Gas` para Artisan
  - `scheduler.rs`: Scheduling de salida (10Hz)
  - `serial.rs`: Salida serie abstracta
  - `uart.rs`: Salida UART concreta
  - `traits.rs`: Traits para formateadores

- **`server/`**:
  - Módulo reservado para expansión futura. Anteriormente contenía un servidor HTTP básico que fue eliminado ya que no estaba integrado en la aplicación principal.

## 4. Diagrama General de Flujo de la Aplicación

El siguiente diagrama ilustra el flujo de datos y control a alto nivel.

```mermaid
graph TD
    subgraph "Interfaz Externa"
        Artisan[Software Artisan]
    end

    subgraph "Firmware LibreRoaster (ESP32)"
        UART(UART)
        InputParser[Input Parser]
        CommandHandler[Command Handler]
        RoasterControl[Control del Tostador]
        
        subgraph "Hardware Abstractions"
            TempSensors[Sensores de Temperatura]
            SSR[Control SSR (Calor)]
            Fan[Control Ventilador]
        end

        OutputFormatter[Output Formatter]
        StatusScheduler[Scheduler de Salida]
    end

    Artisan -- "Comandos (READ, START, OT1, IO3, STOP)" --> UART
    UART --> InputParser
    InputParser -- "ArtisanCommand" --> CommandHandler
    CommandHandler -- "Órdenes" --> RoasterControl
    
    RoasterControl -- "Leer Temps" --> TempSensors
    RoasterControl -- "Ajustar Potencia" --> SSR
    RoasterControl -- "Ajustar Flujo de Aire" --> Fan
    
    TempSensors -- "BT, ET" --> RoasterControl
    
    RoasterControl -- "SystemStatus" --> OutputFormatter
    OutputFormatter -- "time,ET,BT,ROR,Gas" --> UART
    UART --> Artisan
```

## 5. Diagrama de Interacción entre Módulos

Este diagrama muestra cómo los principales módulos de software interactúan entre sí a través del `ServiceContainer`.

```mermaid
graph TD
    subgraph "Tasks (Async)"
        ControlLoop[Control Loop Task - 10Hz]
        ArtisanOutput[Artisan Output Task]
        UartReader[UART Reader Task]
    end

    subgraph "Channels (Embassy Sync)"
        ArtisanCmdChannel[Artisan Command Channel - 8 slots]
        OutputChannel[Output Data Channel - 16 slots]
    end

    subgraph "Core Logic"
        RC[RoasterControl]
        FC[FanController]
        AI[ArtisanInput]
        Formatter[ArtisanFormatter]
    end

    subgraph "Hardware Drivers"
        HD[Hardware Drivers (SPI, GPIO, UART)]
    end

    UartReader -- "Bytes" --> UART
    UART -- "Comandos: READ, START, OT1, IO3, STOP" --> AI
    AI -- "ArtisanCommand" --> ArtisanCmdChannel

    ArtisanCmdChannel -- "Command" --> ControlLoop
    ControlLoop -- "Ejecuta lógica" --> RC
    ControlLoop -- "Lee sensores" --> HD
    ControlLoop -- "Controla" --> FC

    RC -- "SystemStatus" --> Formatter
    Formatter -- "CSV: time,ET,BT,ROR,Gas" --> OutputChannel
    OutputChannel -- "Datos" --> ArtisanOutput
    ArtisanOutput -- "Bytes" --> UART

    HD -- "SPI/I2C/GPIO" --> Sensors[Sensores MAX31856]
    HD -- "PWM" --> SSR[SSR Control]
    HD -- "PWM" --> Fan[Ventilador]
```
