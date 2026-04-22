# Documentación de Hardware - LibreRoaster

> Last Updated: 2026-04-22 (v5.1)

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

## GPIO Pinout

| Pin GPIO | Función | Tipo | Descripción |
|---------|----------|------|-------------|
| 1 | HEAT_DETECTION_PIN | Input | Detección de fuente de calor (feedback SSR) |
| 3 | THERMOCOUPLE_ET_CS_PIN | Output | Chip Select sensor Environment Temperature |
| 4 | THERMOCOUPLE_BT_CS_PIN | Output | Chip Select sensor Bean Temperature |
| 5 | SPI_MOSI_PIN | Output | SPI Master Out |
| 6 | SPI_MISO_PIN | Input | SPI Master In |
| 7 | SPI_SCLK_PIN | Output | SPI Clock |
| 9 | FAN_PWM_PIN | Output | PWM 控制 ventilateur (strapping) |
| 10 | SSR_CONTROL_PIN | Output | PWM SSR (no-strapping) |
| 20 | UART_TX_PIN | Output | UART TX → Artisan |
| 21 | UART_RX_PIN | Input | UART RX ← Artisan |

---

## Especificaciones de Software v5.1

### Timing del Control Loop

| Parámetro | Valor | Notas |
|-----------|-------|-------|
| Control loop interval | 100ms | Frecuencia de actualización |
| Sensor read time | ~160ms | MAX31856 conversión paralelo |
| SSR cycling guard | 100ms | Tiempo mínimo entre cambios SSR |
| PID sample time | 100ms | Actualización PID |
| Watchdog feed | 100ms | Tiempo máximo entre feeds |

### Comandos Artisan Soportados

| Comando | Descripción | Ejemplo |
|---------|-------------|---------|
| READ | Telemetry: ET,BT,Heater,Fan | `READ` → `185.3,201.4,45,80` |
| STATUS / STAT | 18 CSV fields telemetry | `STATUS` |
| OT1 [0-100] | Set heater power % | `OT1 75` |
| OT2 [0-100] | Set fan speed % (auto-cut heater if oor) | `OT2 80` |
| IO3 [0-100] | Set fan speed % | `IO3 75` |
| UP | Increase heater +5% | `UP` |
| DOWN | Decrease heater -5% | `DOWN` |
| START | Begin roast with PID | `START` |
| STOP | Emergency stop | `STOP` |
| PIDGAIN kp ki kd | Set PID gains | `PIDGAIN 2.0 0.25 0.05` |
| SETTARGET temp | Set target temperature | `SETTARGET 210` |
| CHAN;rate | Set communication rate | `CHAN;115200` |
| UNITS;C \| F | Set temperature units | `UNITS;C` |
| FILT;value | Set filter value | `FILT;5` |
| REG | Run regression test | `REG` |

### Nuevos comandos v5.1

```
# Configurar PID gains (tunable en runtime)
PIDGAIN 2.0 0.25 0.05    # Kp, Ki, Kd

# Set target temperature (50-300°C)
SETTARGET 210.5
```

---

## Safety Features

| Feature | Threshold | Action |
|---------|-----------|-------|
| Over-temperature | 260°C | Emergency shutdown, cut heater, max fan |
| Sensor timeout | 1000ms | Fault condition, disable heater |
| Heat detection | SSR feedback | Verify heater is turning on |
| Watchdog | 100ms | System reset if not fed |

---

## Testing sin Hardware

### Tests Unitarios (Host)
```bash
# Ejecutar todos los tests
cargo test --lib

# Tests de parser
cargo test --lib parser

# Tests de formatter
cargo test --lib artisan
```

**Resultado: 167 tests pasando** ✅

### Integración con Artisan

1. Conectar ESP32-C3 via USB
2. Configurar Artisan:
   - Puerto: ttyACM (Linux) o /dev/cu.usbmodem-* (macOS)
   - Baud: 115200
   - Mode: Arduino/RPi
3. Verificar comandos:
   - `READ` → debería retornar temperaturas
   - `OT1 50` → debería cambiar heater output

---

## Hardware Opcional

### Para desarrollo sin hardware:
- ESP32-C3 dev board
- USB cable
- 2x MAX31856 + Type-K thermocouples
- SSR 25A
- Ventilador DC 12V
- Fuente 12V/5A

---

## Notas de Seguridad

⚠️ **ADVERTENCIA**: Este proyecto involucra altas tensiones y temperaturas extremas.

- Solo trabajar en el hardware si se tiene **conocimiento eléctrico adecuado**
- Desconectar siempre la alimentación antes de modificar el circuito
- Usar aislamiento térmico y materiales resistentes al calor
- **No dejar el tostador desatendido durante su funcionamiento**
- Mantener un **extintor de incendios cerca** en todo momento
- Operar en un **área bien ventilada y segura contra incendios**

> El autor y colaboradores **no se hace responsables** de cualquier daño, lesión o pérdida.

---

## Changelog

### v5.1 (2026-04-22)
- SSR cycling guard reducido a 100ms (de 1000ms)
- Nuevos comandos: PIDGAIN, SETTARGET
- Tests actualizados: 167 pasando
- Bug fixes en control loop

### v5.0 (2026-03-11)
- Initial release