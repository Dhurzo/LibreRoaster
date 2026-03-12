# Documentación de Hardware - LibreRoaster

> Last Updated: 2026-03-11 (v5.0)

Este documento describe el hardware utilizado en el proyecto LibreRoaster y sus especificaciones técnicas completas.

## Componentes Principales

### Microcontrolador
| Componente | Especificación | GPIO | Notas |
|------------|----------------|------|-------|
| ESP32-C3 | RISC-V, 32-bit | - | Microcontrolador principal |
| USB-C | Programación y alimentación | - | 500mA mínimo |

### Sensores de Temperatura
| Componente | Especificación | GPIO | Notas |
|------------|----------------|------|-------|
| MAX31856 #1 | Amplificador termopar | SPI | Bean Temperature (BT) |
| MAX31856 #2 | Amplificador termopar | SPI | Environment Temperature (ET) |
| Termopar Type-K | x2 | - | Rango: 0-300°C |

### Control de Potencia
| Componente | Especificación | GPIO | Notas |
|------------|----------------|------|-------|
| SSR | Solid State Relay | GPIO10 | Control de heating element (seguro, no-strapping) |
| GPIO1 | Pin de detección | Input pull-up | Detecta si hay fuente de calor |

### Control de Ventilador
| Componente | Especificación | GPIO | Notas |
|------------|----------------|------|-------|
| Ventilador DC | 12V/24V | GPIO9 | PWM @ 25kHz (strapping, seguro en SPI boot) |
| MOSFET | IRF520 o similar | - | Driver para ventilador |

### Comunicación
| Componente | Especificación | GPIO | Notas |
|------------|----------------|------|-------|
| UART | USB-Serial | GPIO20 (TX), GPIO21 (RX) | Artisan+ communication |
| USB-UART | Adaptador | - | Conexión a PC |

---

## Diagrama de Conexiones

```
ESP32-C3                    MAX31856 #1       MAX31856 #2        SSR          Ventilador
GPIO7  ──────────────────►  SCLK              SCLK               ─            ─
GPIO6  ◄─────────────────   MISO              MISO               ─            ─
GPIO5  ──────────────────►  MOSI              MOSI               ─            ─
GPIO4  ──────────────────►  CS                ─                  ─            ─
GPIO3  ──────────────────►  ─                 CS                 ─            ─
GPIO10 ──────────────────►  ─                 ─                 Control       ─
GPIO1  ◄─────────────────   ─                 ─                 (feedback)    ─
GPIO9  ──────────────────►  ─                 ─                 ─             PWM
GPIO20 ──────────────────►  ─                 ─                 ─             ─   ► TX
GPIO21 ◄─────────────────   ─                 ─                 ─             ─   ◄ RX
3.3V   ──────────────────►  VCC               VCC               ─             ─
GND    ──────────────────►  GND               GND               GND           GND
```

---

## Componentes Electrónicos Adicionales

### Componentes Pasivos - Ubicación

| Componente | Valor | Cantidad | Uso | Ubicación Física |
|------------|-------|----------|-----|------------------|
| Resistencia pull-up | 4.7kΩ | 1 | Línea CS BT | Near GPIO4 (BT CS) |
| Resistencia pull-up | 4.7kΩ | 1 | Línea CS ET | Near GPIO3 (ET CS) |
| Resistencia pull-up | 4.7kΩ | 1 | Línea CLK SPI | Near GPIO7 (SPI CLK) |
| Resistencia pull-up | 10kΩ | 1 | Pin detección calor | Near GPIO1 (Heat Detect) |
| Resistencia pull-up | 10kΩ | 1 | Pin control SSR | Near GPIO10 (SSR Control) |
| Condensador ceramic | 100nF | 1 | Decoupling VCC | Near ESP32-C3 VCC pin |
| Condensador ceramic | 100nF | 1 | Decoupling MAX31856#1 | Near MAX31856 #1 VCC |
| Condensador ceramic | 100nF | 1 | Decoupling MAX31856#2 | Near MAX31856 #2 VCC |
| Condensador ceramic | 100nF | 1 | Decoupling MOSFET | Near MOSFET Gate |
| Condensador ceramic | 100nF | 1 | Decoupling 5V regulator | Near 5V regulator output |
| Condensador electrolytic | 10µF | 1 | Filtrado alimentación | Power input filtering |
| Condensador electrolytic | 10µF | 1 | Filtrado ventilador | Fan motor supply filtering |

### Componentes Activos - Ubicación

| Componente | Especificación | Cantidad | Uso | Ubicación Física |
|------------|----------------|----------|-----|------------------|
| Optoacoplador | PC817 | 1 | Aislamiento GPIO10-SSR | Near SSR input terminals |
| MOSFET | IRF520N | 1 | Driver ventilador DC | Near ESP32-C3 GPIO9 and fan |
| Diodo Schottky | 1N5819 | 1 | Protección voltaje inverso | Across MOSFET D-S pins |

### Componentes de Potencia - Ubicación

| Componente | Especificación | Uso | Ubicación Física |
|------------|----------------|-----|------------------|
| SSR | 25A, 250V AC | Control heating element | High-voltage area, away from low-voltage logic |
| Fusible | 10A, медленный | Protección circuito heating | In-line on AC hot wire |
| Terminal block | 2 posiciones, 10A | Conexión alta potencia | Near SSR output |

---

## Layout Físico del PCB

```
┌─────────────────────────────────────────────────────────────┐
│                     PCB LAYOUT - VISTA SUPERIOR             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    LOW VOLTAGE ZONE                  │    │
│  │                                                     │    │
│  │  ┌──────────┐     ┌──────────┐     ┌──────────┐    │    │
│  │  │ ESP32-C3 │────►│  MOSFET  │────►│   FÁN    │    │    │
│  │  │          │     │  IRF520N │     │          │    │    │
│  │  │ GPIO1───►│     │   GATE   │     │  +12V    │    │    │
│  │  │ GPIO10──►│     │  D   S   │     │  GND     │    │    │
│  │  │ GPIO9───►│     │  1N5819  │     │          │    │    │
│  │  └──────────┘     └──────────┘     └──────────┘    │    │
│  │       │               │                                │
│  │  ┌────┴────┐    ┌─────┴─────┐                         │    │
│  │  │ 100nF   │    │ 10kΩ      │    ┌──────────┐        │    │
│  │  │ C1      │    │ R4        │    │ MAX31856 │        │    │
│  │  └─────────┘    └───────────┘    │  #1 (BT) │        │    │
│  │                                   └────┬─────┘        │    │
│  │  ┌──────────┐     ┌──────────┐         │             │    │
│  │  │ MAX31856 │     │  100nF   │    ┌────┴────┐        │    │
│  │  │  #2 (ET) │     │   C2     │    │ 4.7kΩ   │        │    │
│  │  └──────────┘     └──────────┘    │  R1      │        │    │
│  │                                   └──────────┘        │    │
│  └─────────────────────────────────────────────────────┘    │
│                        │                                    │
│  ═══════════════════════════════════════════════════════════│    │
│                        │                                    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   HIGH VOLTAGE ZONE                  │    │
│  │                                                     │    │
│  │     AC IN ──┬── Fusible ──┬── SSR ──┬── Heating    │    │
│  │             │    (10A)    │         │  Element    │    │
│  │             │             │         │             │    │
│  │         ┌───┴───┐         │         └─────────────┘    │    │
│  │         │PC817   │         │                           │    │
│  │         │Optocou │         │                           │    │
│  │         │pler    │         │                           │    │
│  │         └────────┘         │                           │    │
│  │              │             │                           │    │
│  │         GPIO10─┘           │                           │    │
│  │                                                     │    │
│  │  ┌──────────┐                                      │    │
│  │  │Terminal  │                                      │    │
│  │  │Block     │                                      │    │
│  │  │  AC      │                                      │    │
│  │  └──────────┘                                      │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘

ZONAS:
┌─────────────────────────┐  LOW VOLTAGE (3.3V/5V) - Lógica
═════════════════════════  SEPARACIÓN GALVÁNICA
└─────────────────────────┘  HIGH VOLTAGE (110V/220V AC)
```

### Resumen de Zonas del PCB

| Zona | Componentes | Voltaje | Notas |
|------|-------------|---------|-------|
| **Low Voltage** | ESP32-C3, MOSFET, MAX31856, sensores | 3.3V/5V/12V | Zona segura para lógica |
| **Optoacoplador** | PC817 | Aislamiento | Barrera galvánica |
| **High Voltage** | SSR, fusible, terminal block | 110V/220V AC | Zona de peligro |

---

## Configuración de Pines

```rust
// src/config/constants.rs

// Pines GPIO (Nota: GPIO2, GPIO8 son strapping - NO USADOS para evitar problemas de boot)
pub const SPI_SCLK_PIN: u8 = 7;
pub const SPI_MOSI_PIN: u8 = 5;
pub const SPI_MISO_PIN: u8 = 6;
pub const THERMOCOUPLE_BT_CS_PIN: u8 = 4;
pub const THERMOCOUPLE_ET_CS_PIN: u8 = 3;
pub const SSR_CONTROL_PIN: u8 = 10;   // GPIO10 (no-strapping, seguro)
pub const HEAT_DETECTION_PIN: u8 = 1;
pub const FAN_PWM_PIN: u8 = 9;        // GPIO9 (strapping, seguro en modo SPI boot)
pub const UART_TX_PIN: u8 = 20;  // GPIO20 - TX
pub const UART_RX_PIN: u8 = 21;  // GPIO21 - RX

// PWM
pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000;  // 25kHz para ventilador DC
pub const SSR_PWM_FREQUENCY_HZ: u32 = 1;       // 1Hz para SSR
pub const FAN_LEDC_CHANNEL: u8 = 0;            // Canal 0 para ventilador
pub const SSR_LEDC_CHANNEL: u8 = 1;            // Canal 1 para SSR
```

### Configuración de Pines Completa

| GPIO | Función | Descripción |
|------|---------|-------------|
| 1 | HEAT_DETECTION_PIN | Entrada digital - detección de fuente de calor |
| 3 | THERMOCOUPLE_ET_CS_PIN | Salida digital - Chip Select sensor ambiental |
| 4 | THERMOCOUPLE_BT_CS_PIN | Salida digital - Chip Select sensor de granos |
| 5 | SPI_MOSI_PIN | Salida SPI - Master Out |
| 6 | SPI_MISO_PIN | Entrada SPI - Master In |
| 7 | SPI_SCLK_PIN | Salida SPI - Clock |
| 9 | FAN_PWM_PIN | Salida PWM - control de ventilador (strapping) |
| 10 | SSR_CONTROL_PIN | Salida PWM - control de SSR (no-strapping) |
| 20 | UART_TX_PIN | Salida UART - transmisión a Artisan |
| 21 | UART_RX_PIN | Entrada UART - recepción desde Artisan |

### Pines NO Utilizados (Strapping - Potencialmente Riesgosos)

| GPIO | Razón |
|------|-------|
| 2 | Strapping pin - evita problemas de boot |
| 8 | Strapping pin - evita problemas de boot |

---

## ESP32-C3 (RISC-V)
- **Arquitectura:** RISC-V de 32 bits, single-core
- **Frecuencia:** Hasta 160 MHz
- **Memoria:** 400 KB SRAM, 4 MB Flash
- **Conectividad:** Wi-Fi 802.11n, Bluetooth 5
- **Ventajas:** Bajo consumo, excelente soporte para Rust/Embassy

## MAX31856 (x2)
- **Tipo:** Convertidor termopar a digital con SPI
- **Termopares:** Tipo-K (estándar para café)
- **Precisión:** ±2°C
- **Resolución:** 24-bit (0.25°C)
- **Interfaces:** SPI compartido

---

## Módulo PWM - Estado Actual

**NO se requiere módulo adicional.** El ESP32-C3 tiene periférico LEDC integrado:

```rust
// src/hardware/fan.rs - Implementación del PWM
pub fn with_ledc(ledc_peripheral: LEDC, gpio9: GPIO9) -> Result<Self, FanError> {
    let mut ledc = Ledc::new(ledc_peripheral);
    ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);
    
    let mut timer = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    timer.configure(timer::config::Config {
        duty: timer::config::Duty::Duty8Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_hz(FAN_PWM_FREQUENCY_HZ),  // 25kHz
    })?;
    
    // Channel0 para ventilador (GPIO9)
    let mut channel = ledc.channel(channel::Number::Channel0, gpio9);
    channel.configure(channel::config::Config {
        timer: &timer,
        duty_pct: 0,
        drive_mode: esp_hal::gpio::DriveMode::PushPull,
    })?;
}
```

### Canales LEDC Utilizados

| Canal | Periférico | Frecuencia | Resolución | GPIO |
|-------|-----------|------------|------------|------|
| Channel0 | Ventilador | 25kHz | 8-bit (0-255) | GPIO9 |
| Channel1 | SSR | 1Hz | 8-bit (0-255) | GPIO10 |

---

## Sistema de Ventilación

### Ventilador DC Controlado por PWM
- **Tipo:** Ventilador DC de alta velocidad
- **Control:** PWM a 25 kHz (frecuencia óptima para motores DC)
- **Resolución:** 8-bit (0-255 niveles)
- **Pin de Control:** `FAN_PWM_PIN: GPIO9` (strapping, pero seguro en modo SPI boot)
- **Frecuencia:** 25,000 Hz (ultrasónico, sin ruido audible)

### Circuito del Ventilador con MOSFET

El ESP32-C3 no puede fornecer la corriente necesaria para el ventilador DC, por lo que se usa un MOSFET como driver:

```
                    +12V DC
                      │
              ┌──────┴──────┐
              │   FUSIBLE   │  (10A)
              └──────┬──────┘
                     │
              ┌──────┴──────┐
              │   VENTILADOR│  (DC 12V/24V, 200mA-2A)
              │     DC      │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │    MOSFET   │  (IRF520N)
              │             │
   GPIO9 ─────┤    GATE     │  ←── PWM 25kHz desde ESP32-C3 (GPIO9)
              │             │
              ├────┬────────┤
              │    │        │
              │  D │        │ S
              │    │        │
              └──┬─┴────┬───┘
                 │     │
                 │     └─────── GND
                 │
              ┌──┴───┐
              │ DIODO│  (1N5819)
              │SCHOTT│  flyback
              │  KY  │
              └──┬───┘
                 │
               GND
```

### Conexión del MOSFET (IRF520N)

| Pin MOSFET | Conexión |
|------------|----------|
| **GATE (G)** | GPIO9 del ESP32-C3 (PWM 25kHz) |
| **DRAIN (D)** | Negativo del ventilador DC |
| **SOURCE (S)** | GND (tierra) |
| **Diodo Flyback** | Entre D y S (protección contra picos de voltaje) |

### ¿Por qué se necesita el MOSFET?

1. **El GPIO del ESP32-C3 solo puede fornecer ~20mA**
2. **El ventilador DC típicamente consume 200mA-2A**
3. **El MOSFET actúa como interruptor controlado por PWM**

---

## Control de Calor

### SSR (Solid State Relay)
- **Tipo:** Relevador de estado sólido para AC
- **Control:** PWM a 1 Hz para regulación de potencia
- **Pin de Control:** `SSR_CONTROL_PIN: GPIO10` (no-strapping, seguro)
- **Detección de Presencia:** `HEAT_DETECTION_PIN: GPIO1`
- **Seguridad:** Sistema de detección de fuente de calor

**Características:**
- Control de potencia 0-100% mediante ciclo de trabajo
- Detección de conexión/desconexión del elemento calefactor
- Apagado automático en caso de fallo

---

## Comunicación y Datos

### USB CDC (Recomendado)

El ESP32‑C3 soporta comunicación USB CDC nativa, eliminando la necesidad de un adaptador USB‑UART. Conecte el puerto USB directamente a la PC para comunicación con Artisan.

| Parámetro | Valor |
|-----------|-------|
| Velocidad | 115,200 baudios |
| Protocolo | CSV compatible con Artisan |
| Formato | `ET,BT,HEATER,FAN` |
| Frecuencia | 10 Hz (100ms entre muestras) |

### UART para Artisan+

| Parámetro | Valor |
|-----------|-------|
| Velocidad | 115,200 baudios |
| Protocolo | CSV compatible con Artisan |
| TX (T→PC) | GPIO20 |
| RX (P→T) | GPIO21 |
| Formato | `ET,BT,HEATER,FAN` |
| Frecuencia | 10 Hz (100ms entre muestras) |

### Comandos ARTISAN+ Soportados

| Comando | Formato | Descripción |
|---------|---------|-------------|
| READ | `READ` | Retorna: `ET,BT,HEATER,FAN` |
| STATUS/STAT | `STATUS` o `STAT` | Telemetría de automatización (18 campos, incluye métricas de seguridad) |
| REG | `REG` | Disparador de regresión por sobretemperatura (pruebas de seguridad) |
| START | `START` | Inicia tostado y streaming continuo |
| OT1 x | `OT1 <0-100>` | Control calentador manual (%) |
| IO3 x | `IO3 <0-100>` | Control ventilador (%) |
| OT2 x | `OT2 <0-100>` | Control ventilador con decimales (%) |
| STOP | `STOP` | Parada de emergencia |

### Flujo de Comunicación

```
┌──────────┐    GPIO21    ┌──────────────────┐    ┌─────────────────┐
│  Artisan │  ◄────────── │ UART RX (GPIO21) │───►│ parse_artisan   │
│  (PC)    │              │ (driver.rs)      │    │ (parser.rs)     │
└────┬─────┘              └──────────────────┘    └────────┬────────┘
     │                                                        │
     │ TX (Comandos)                                         │ ArtisanCommand
     ▼                                                        ▼
┌──────────┐    GPIO20    ┌──────────────────┐    ┌─────────────────┐
│  Artisan │  ──────────► │ UART TX (GPIO20) │◄───│ ArtisanFormatter│
│  (PC)    │              │ (driver.rs)      │    │ (output/artisan)│
└──────────┘              └──────────────────┘    └─────────────────┘
                                │
                                ▼
                      "ET,BT,HEATER,FAN\r\n"
```

### Verificación de Integración

```bash
# Conectar al puerto serie
minicom -D /dev/ttyUSB0 -b 115200

# Verificar respuesta
> READ
< 25.1,24.8,0.0,0    (ET,BT,SSR,Fan)

> START
< 0.00,25.2,24.9,0.0,0
< 0.10,25.4,25.1,0.2,5
< 0.20,25.8,25.5,0.6,12
...
```

---

## Especificaciones de Operación

### Rangos de Temperatura
- **Temperatura Objetivo:** 225°C (típico para café)
- **Límite Máximo Seguro:** 250°C
- **Apagado de Emergencia:** 260°C
- **Tiempo de Conversión:** 160 ms por lectura

### Frecuencias de Control
- **Fan PWM:** 25 kHz
- **SSR PWM:** 1 Hz
- **Muestreo de Temperatura:** 6.25 Hz (cada 160 ms)
- **Envío de Datos Artisan:** 10 Hz

---

## Circuitos de Seguridad

### Protecciones Implementadas
- **Sobretemperatura:** Apagado automático > 260°C
- **Timeout de Sensor:** Detección de fallo > 1000 ms
- **Detección de Calor:** Verificación cada 5 segundos
- **Paro de Emergencia:** Apagado inmediato del SSR

### Monitoreo de Hardware
- **Estado del SSR:** Detección de conexión del elemento calefactor
- **Integridad de Sensores:** Validación de lecturas de temperatura
- **Comunicación:** Heartbeat con software Artisan+

---

## Requisitos de Alimentación

### Fuente de Poder Principal
- **Voltaje:** 5V DC para el ESP32-C3
- **Consumo:** ~150 mA máx (incluyendo periféricos)
- **Recomendado:** Fuente de 5V/1A para margen de seguridad

### SSR y Elemento Calefactor
- **Voltaje de Control:** 3.3V desde GPIO
- **Potencia del Elemento:** Variable según diseño del tostador
- **Tipo:** Corriente Alterna (AC) típica de 110V/220V

---

## Lista de Compra Completa

```
Cantidad  │ Componente                    │ Precio estimado
───────────┼───────────────────────────────┼─────────────────
1          │ ESP32-C3 Dev Kit              │ ~$5-10
2          │ MAX31856                      │ ~$2-4 c/u
2          │ Termopar Type-K               │ ~$3-5 c/u
1          │ SSR 25A 250V                  │ ~$8-15
1          │ Ventilador DC 12V/24V         │ ~$10-20
1          │ MOSFET IRF520N                │ ~$1-2
1          │ Optoacoplador PC817           │ ~$0.50
1          │ Protoboard o PCB              │ ~$5-10
-          │ Cables y conectores           │ ~$5-10
1          │ Fusible 10A медленный         │ ~$2-3
1          │ Portafusibles                 │ ~$1-2
-          │ Resistencias y condensadores  │ ~$3-5
───────────┼───────────────────────────────┼─────────────────
           │ TOTAL ESTIMADO                │ ~$50-80
```

---

## Componentes Opcionales

### Display LCD (Futuro)
- **Tipo:** I2C LCD 16x2 o 20x4
- **Propósito:** Visualización local de estado
- **Pines:** GPIO4/GPIO5 (I2C) - **Pendiente de implementación**

### Botonera de Control (Futuro)
- **Funciones:** Start/Stop, Emergencia, Ajuste manual
- **Pines:** GPIO12-GPIO15 - **Pendiente de implementación**

---

## Integración ARTISAN+ - Estado

| Feature | Estado | Descripción |
|---------|--------|-------------|
| **Lectura temperaturas** | ✅ Funcional | BT y ET via MAX31856 |
| **Control de potencia** | ✅ Funcional | SSR via OT1 |
| **Control de ventilador** | ✅ Funcional | Fan via IO3 |
| **Inicio/Parada** | ✅ Funcional | START/STOP |
| **Streaming datos** | ✅ Funcional | 10Hz, formato CSV |
| **READ status** | ✅ Funcional | ET,BT,Power,Fan |

---

## Notas de Diseño

- **ESD Protection:** Considerar protección contra descargas electrostáticas en interfaces expuestas
- **Aislamiento:** Optoacopladores recomendados para SSR y comunicación externa
- **Filtrado:** Capacitores de desacoplo cerca del ESP32-C3
- **Layout:** Mantener señales analógicas lejos de fuentes de ruido digital

---

## Verificación de Integración Completa

### Checklist de Hardware

- [ ] ESP32-C3 programado con firmware LibreRoaster
- [ ] MAX31856 #1 conectado (GPIO4 CS, SPI completo)
- [ ] MAX31856 #2 conectado (GPIO3 CS, SPI completo)
- [ ] Termopares Type-K conectados a ambos MAX31856
- [ ] SSR conectado a GPIO10 (con optoacoplador)
- [ ] Ventilador conectado a GPIO9 (con MOSFET)
- [ ] Detección de calor en GPIO1
- [ ] USB-UART conectado a GPIO20/GPIO21

### Prueba de Funcionalidad

```bash
# 1. Monitorear serial
espflash monitor

# 2. Verificar inicio (esperar mensajes INFO)
# INFO: LibreRoaster started - Artisan+ UART control ready
# INFO: Roaster is ready!

# 3. Conectar Artisan
# Device → Device Configuration
# Serial Port: Puerto USB-UART
# Baud Rate: 115200
# Arduino/RPi: ✓
# Extra: ✓ "机电"

# 4. En Artisan, enviar START
# Verificar que los gráficos se actualizan

# 5. Probar control manual
# Enviar: OT1 50    (50% potencia)
# Enviar: IO3 75    (75% ventilador)

# 6. Parada de emergencia
# Enviar: STOP
```

### Indicadores de Éxito

| Indicador | Esperado |
|-----------|----------|
| Lectura BT/ET | Temperaturas estables, ~25°C ambiente |
| Respuesta a START | Streaming continuo a 10Hz |
| Respuesta a OT1 | SSR output cambia % |
| Respuesta a IO3 | Fan speed cambia % |
| Respuesta a STOP | Sistema en estado Idle |

---

## Notas de Seguridad

⚠️ **ADVERTENCIA**: Este proyecto involucra altas tensiones y temperaturas extremas.

- Solo trabajar en el hardware si se tiene **conocimiento eléctrico adecuado**
- Desconectar siempre la alimentación antes de modificar el circuito
- Usar aislamiento térmico y materiales resistentes al calor
- **No dejar el tostador desatendido durante su funcionamiento**
- Mantener un **extintor de incendios cerca** en todo momento
- Operar en un **área bien ventilada y segura contra incendios**

> El autor y colaboradores **no se hacen responsables** de cualquier daño, lesión o pérdida.
