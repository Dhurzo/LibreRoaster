# Artisan USB CDC Verification Plan

## Objetivo
Verificar que la conexión USB CDC con Artisan funciona correctamente y que el cambio de canal de comunicación funciona según lo diseñado.

## Requisitos previos

### Hardware
- Placa ESP32-C3 con firmware flasheado
- Cable USB conectado al puerto nativo USB (no UART)

### Software
- Artisan instalado (versión reciente)
- Terminal serie (minicom, putty, screen, etc.)

## Pruebas de conexión básica

### Paso 1: Verificar que el dispositivo aparece

```bash
# En Linux
ls -la /dev/ttyACM*
# o
ls -la /dev/ttyUSB*

# Deberías ver algo como:
# /dev/ttyACM0
```

```bash
# En macOS
ls /dev/tty.*
# Buscar: /dev/tty.usbmodemXXX
```

### Paso 2: Conectar con terminal

```bash
# Linux
minicom -D /dev/ttyACM0 -b 115200

# o con picocom
picocom -b 115200 /dev/ttyACM0

# o screen
screen /dev/ttyACM0 115200
```

### Paso 3: Verificar respuesta READ

En la terminal, escribir:
```
READ
```

Debes presionar Enter (CR). El dispositivo debería responder:
```
120.3,150.5,75.0,25.0
```
(o valores diferentes según las temperaturas actuales)

## Pruebas de comandos Artisan

### Comandos a probar

| Comando | Acción | Respuesta esperada |
|---------|--------|-------------------|
| `READ` | Leer temperaturas | `ET,BT,Power,Fan` |
| `START` | Iniciar roasting | Sin respuesta inmediata (habilita streaming) |
| `OT1 50` | Heater al 50% | Sin respuesta (si es el primer comando del canal) |
| `IO3 30` | Fan al 30% | Sin respuesta (si es el primer comando del canal) |
| `STOP` | Emergencia | Sin respuesta (detiene outputs) |

### Comandos que deben dar error

| Comando | Error esperado |
|---------|----------------|
| `BOGUS` | `ERR unknown_command BOGUS` |
| `OT1 150` | `ERR invalid_value OT1 150` |
| `OT1 ABC` | `ERR parse_error OT1 ABC` |

## Pruebas de cambio de canal

### Escenario 1: USB → UART

1. Conectar por USB, enviar `READ` → funciona
2. Desconectar USB, conectar UART0
3. Enviar `READ` por UART → debería cambiar el canal activo a UART
4. Ver logs: `Artisan command received on UART, switching active channel to UART`

### Escenario 2: Timeout (60 segundos)

1. Conectar por USB, enviar `READ` → funciona
2. Esperar 60 segundos sin enviar comandos
3. Verificar log: `No artisan commands for 60s, switching active channel to None`
4. Enviar `READ` nuevamente → debe funcionar

### Escenario 3: Ignorar comandos en canal inactivo

1. Conectar por USB, enviar `READ` → USB es el canal activo
2. Conectar también por UART (si es posible)
3. Enviar comando por UART → debe ser ignorado
4. Ver log: `Ignoring artisan command on UART, active channel is USB`

## Configuración en Artisan

### Configurar Artisan para USB

1. Abrir Artisan
2. Ir a `Config` → `Device`
3. Seleccionar:
   - **Type:** `Arduino` o `TC4` (depende de tu configuración)
   - **Port:** `/dev/ttyACM0` (Linux) o puerto USB correspondiente
   - **Baudrate:** `115200`
4. Conectar y verificar que las temperaturas aparecen

### Si Artisan no conecta

1. Verificar que el dispositivo está conectado: `ls /dev/ttyACM*`
2. Verificar permisos: `sudo usermod -a -G dialout $USER`
3. Probar con terminal primero antes de Artisan
4. Revisar logs del dispositivo (si hay output de debug)

## Verificación de logs

El firmware debería mostrar logs cuando:

1. **Canal se activa:**
   ```
   [INFO] Artisan command received on USB, switching active channel to USB
   ```

2. **Timeout:**
   ```
   [INFO] No artisan commands for 60s, switching active channel to None
   ```

3. **Comando ignorado:**
   ```
   [INFO] Ignoring artisan command on UART, active channel is USB
   ```

## Troubleshooting

### El dispositivo no aparece como /dev/ttyACM*

- Verificar que el cable USB está conectado
- Probar otro cable USB (algunos cables son solo de carga)
- Verificar que la placa está encendida

### Artisan no puede abrir el puerto

- Verificar permisos de usuario
- Cerrar otras aplicaciones que usen el puerto
- Verificar que no hay otro proceso usando el puerto: `lsof /dev/ttyACM0`

### No hay respuesta a los comandos

- Verificar que la terminal está configurada para enviar CR (`\r`)
- Probar con minicom configurado para CR
- Verificar que el baudrate es 115200
- Revisar si el LED de actividad USB parpadea

### Los comandos UART funcionan pero USB no

- Verificar que USB está inicializado en el código
- Verificar que el peripheral USB_SERIAL_JTAG está configurado correctamente
