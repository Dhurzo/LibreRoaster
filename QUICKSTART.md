# LibreRoaster Quick Start

One-page reference for getting up and running with LibreRoaster and Artisan.

## 🔌 Flash

1. **Build the firmware**
   ```bash
   cargo espflash flash --release --monitor
   ```

2. **See FLASH_GUIDE.md for detailed flashing instructions**

---

## 🔗 Connect

1. Connect ESP32-C3 to computer via USB
2. Device will appear as USB CDC virtual COM port
3. Note the COM port (Windows) or device path (Linux/macOS)
4. Default baud rate: **115200**

---

## ⚙️ Configure

**Artisan Serial Settings:**
- Port: Your device's COM port
- Baud Rate: 115200
- Timeout: 5 seconds (adjust as needed)

**Device Communication:**
- Artisan sends commands: `READ`, `ON`, `OFF`, `SET`
- Device responds with temperature data and status

---

## ☕ Start Roast

1. Open Artisan
2. Select correct port and set baud to 115200
3. Click **Connect** button
4. Verify temperature readings appear
5. Click **ON** to start roast session
6. Monitor temperatures and adjust heater/fan as needed

---

## 🔧 If Things Don't Work

**No connection?**
- Check USB cable (data lines, not charge-only)
- Verify correct COM port
- Set baud rate to 115200

**Stale readings?**
- Try different USB port
- Reduce cable length
- Check UART logs for errors

**See TROUBLESHOOTING_GUIDE.md for detailed diagnostics.**

---

*LibreRoaster + Artisan v1.8*
