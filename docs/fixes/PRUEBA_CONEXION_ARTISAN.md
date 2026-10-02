# Prueba de conexión con Artisan (placa ESP32-C3 sola)

**Objetivo:** validar que Artisan se conecta, dibuja ET/BT y que los sliders llegan al firmware. Se hace con la placa sola: sin termopares, sin resistencia y sin ventilador. La temperatura es una curva simulada.

**Estado:** análisis basado en código, actualizado a `develop` @ `3584c0a` (2026-10-02). Aún no se ha probado en hardware.

> Cambios respecto a la versión anterior de esta guía: la feature `no-heat-sense` **ya no existe** (el heat-sense es ahora opt-in con `heat-sense`, así que la build por defecto no bloquea el calentador), los logs **ya no salen por el USB** (van por UART0), `STOP` sigue siendo parada con latch, y `PID;ON` ya no quita el latch.

---

## 1. Compilar y flashear

```bash
cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"
```

| Feature | Por qué |
|---|---|
| `simulated-sensors` | Sin chips MAX31856 el firmware **aborta el arranque**. Con los chips pero sin termopares, detectan "termopar abierto", BT pasa a NaN y salta el latch de seguridad. La simulación se salta el SPI y genera la curva. |
| *(no añadir `heat-sense`)* | Sin circuito en GPIO1, la feature `heat-sense` bloquearía el calentador a partir del 50 %. La build por defecto no interpreta GPIO1. |

> ⚠️ **No conectar la resistencia en esta fase.**

---

## 2. Antes de abrir Artisan

1. **Cierra `espflash monitor`**: si sigue abierto, Artisan dará "Unable to open serial port".
2. Identifica el puerto:
   ```bash
   ls /dev/ttyACM*
   ```
   - **Aparece** `/dev/ttyACM0`: es el USB nativo. Usa ese. Los logs del firmware van por UART0, así que **no** aparecen aquí.
   - **No aparece:** la placa solo tiene el conversor USB-serie (UART0). Usa `/dev/ttyUSB0`. En ese caso los logs `WARN` del firmware y la salida del bootloader **comparten el cable** con Artisan (ver §7.1), y es probable que la placa se reinicie cada vez que Artisan abra el puerto.

---

## 3. Configuración de Artisan

| Dónde | Valor |
|---|---|
| Config → Device | **TC4** (Arduino TC4). ET = canal **1**, BT = canal **2**. Sin dispositivos extra. |
| Config → Port | El puerto del paso 2, **115200 8N1**. Timeout 0,4 s por defecto: vale. |
| Config → Events (sliders) | Comando serie `OT1;{}` para el calentador e `IO3;{}` para el ventilador. |
| Botón extra (recomendado) | Uno que envíe `PID;OFF`: la salida limpia de cualquier bloqueo. |
| Config → Serial (log) | **Activarlo**: es la única forma de ver calentador y ventilador (ver §6). |
| PID de Artisan | Si vas a probar PID ON: en el diálogo PID poner **Input = 2** (BT). Con el valor por defecto (1) Artisan hace que el firmware regule **ET**. Desactivar "PID ON at CHARGE". |

---

## 4. Orden de la prueba

- [ ] 1. Flashear (§1).
- [ ] 2. Cerrar el monitor (§2).
- [ ] 3. Conectar Artisan (botón ON).
- [ ] 4. **Comprobar el handshake:** no debe aparecer "Arduino could not set channels/units/filters".
- [ ] 5. **Comprobar las curvas:** ET y BT se dibujan con valores coherentes (ver §5).
- [ ] 6. **Mover los sliders** `OT1` / `IO3` y comprobar en el log serie que llegan.
- [ ] 7. *(Opcional)* Probar **PID ON**, solo cuando hayan pasado **2 min desde el último arranque** (ver §7.2).
- [ ] 8. Terminar con **`PID;OFF`**.

---

## 5. Qué deberías ver

### La curva simulada (cuenta desde el arranque de la placa)

| Tiempo desde el arranque | BT (°C) | ET (°C) |
|---:|---:|---:|
| 0 s | 25 | 25 |
| 30 s | 80 | 100 |
| 60 s | 120 | 150 |
| 2 min | 150 | 180 |
| 4 min | 190 | 220 |
| 7 min | 215 | 240 |
| 9 min | 225 | 250 |
| ≥ 10 min | **225 (fija)** | **250 (fija)** |

### ¿Se reinició la placa al abrir el puerto?

- Si Artisan **empieza en ≈25 °C** al conectar, la placa **se reinició** al abrir el puerto, y la curva arranca desde ese momento.
- Si **empieza a mitad de curva** o fija en 225 °C, **no se reinició**.

### Respuesta a `READ`

- Sin PID: `AMB,ET,BT,0.0,0.0` (5 campos). En modo simulado `AMB` es `0.0` (no hay chip del que leer la unión fría).
- Con PID: `AMB,ET,BT,0.0,0.0,HEATER,FAN,SV` (8 campos).
- La latencia esperada es < 120 ms: en modo simulado no hay espera de conversión y el tick es de ≈100–120 ms.

---

## 6. Calentador y ventilador no se ven en Artisan (es normal)

El TC4 original responde `AMB,ET,BT,Heater,Fan,SV` (6 campos). LibreRoaster mete dos `0.0` antes, así que calentador, ventilador y SV quedan desplazados a las posiciones 5, 6 y 7 (`comm.py:7119-7126`).

- Sin dispositivos extra, Artisan solo lee ET y BT, así que **no afecta a la prueba**.
- Si añades `+ArduinoTC4_56` verás calentador/ventilador **a 0**. Si añades `+ArduinoTC4_78`, el **"SV" será en realidad el % del calentador**.
- **Para ver los valores reales, usa el log serie de Artisan**, que muestra la línea completa.

---

## 7. Cosas que parecen fallos y no lo son

### 7.1 Muestras sueltas a `-1`
Solo posibles si Artisan va por **UART0** (`/dev/ttyUSB0`): ahí los logs `WARN` comparten el cable y uno que caiga en la ventana de `READ` da una muestra `-1`. Por USB nativo ya no pasa (los logs van por UART0). No es un problema de conexión: Artisan vacía el buffer antes de cada `READ`.

### 7.2 Latch por velocidad de subida (RoR) en los primeros 2 minutos
- La curva sube 25→120 °C en 60 s (≈1,6 °C/s). Si el guard de RoR está armado, hace latch en 1–2 s.
- **Lo arman** solo los modos en que el **firmware** controla el calentador: PID ON (`PID;ON`), `START` y enviar un SV (`PID;SV`). Con sliders (`OT1`) el guard está desarmado, también después de un `START`.
- **Regla:** espera **≥ 2 min desde el último arranque**, incluido un reinicio al abrir el puerto, antes de tocar PID/SV/START.

### 7.3 Bloqueos en una sesión larga

| Situación | Qué pasa | Cuándo |
|---|---|---|
| `OT1 > 0` con la curva ya fija en 225 °C (≥ 10 min) | **Nada**: BT 225 y ET 250 planas con BT > 60 °C se consideran equilibrio térmico (exención de sonda atascada en manual) | — |
| Parar el muestreo de Artisan con `OT1 > 0` | Latch por comunicación (comms-idle) | 15 s |
| PID ON con un SV más de 5 °C por encima de la BT fija | Latch "Probe stuck" (PID de firmware) | 2 min |
| `OT1 > 0` acumulado en manual | `Maximum roast time exceeded` (presupuesto manual) | 90 min |
| `START`/`PID;ON` + 30 min | `Maximum roast time exceeded` (presupuesto de tueste) | 30 min desde START |

### 7.4 `STOP`, `PID;OFF`, `PID;ON`, `START`
- **`STOP`** es una parada de emergencia con latch: después **rechaza los sliders**.
- **`PID;OFF`** es la salida limpia: quita el latch y no vuelve a calentar.
- **`PID;ON`** ya **no** quita el latch (lo rechaza con `ERR …fault_condition_active`).
- **`START`** quita el latch pero **vuelve a calentar** con el PID hacia 225 °C (o hacia el objetivo de `PREHEAT` si venías de ahí). No lo uses para recuperar.
- Después de `PID;OFF` el **ventilador se queda al 100 %** hasta que BT baje de 60 °C **o hasta el siguiente `OT1 > 0`**, que devuelve el control del ventilador. Con la BT simulada (nunca baja de 60 °C) el `IO3` parece muerto hasta que muevas el `OT1`.

---

## 8. Si algo falla de verdad

| Síntoma | Causa probable | Qué hacer |
|---|---|---|
| "Unable to open serial port" | `espflash monitor` abierto, o puerto equivocado | Cerrar el monitor; revisar §2 |
| "Arduino could not set channels…" en bucle | Llegan líneas sin `#` en el handshake (solo plausible por UART0: logs o bootloader) | Activar el log serie y ver qué líneas llegan; probar por USB nativo |
| Todo a `-1` de forma continua | Puerto o baudios incorrectos, o la placa reiniciándose en bucle | Mirar `espflash monitor` (con Artisan cerrado) y comprobar que arranca |
| BT/ET intercambiadas | Canales de Artisan cambiados | ET = 1, BT = 2 |
| `OT1` no hace nada | Hay un latch activo, o el PID hizo latch antes de los 2 min | Enviar `PID;OFF`; mirar si sale `ERR safety_fault …` en el log serie |
| `OT1 1–4` muestra heater 0 | 1–4 % no es entregable (duty mínimo = medio ciclo de red, 819 ticks) | Usar ≥ 5 %. Tras R1 el paso `OT1 5`/`UP` desde 0 sí entrega potencia |
| `OT1` ≥ 50 % vuelve a 0 | La build lleva `heat-sense` sin circuito en GPIO1 | Reflashear con las features del §1 |

---

## 9. Comprobación previa opcional (sin Artisan)

El script `scripts/hw_virtual_roast.py` hace un tueste virtual completo por serie. Revisa en el script qué puerto espera: está pensado para UART.
