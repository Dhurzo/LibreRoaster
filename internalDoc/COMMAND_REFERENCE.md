# LibreRoaster Command Reference

> **Analysis (2026-02-05):** Key command facts for roasters.
> - Commands: READ, OT1, IO3, UP, DOWN, START, STOP (7 total)
> - Format: ASCII text terminated with CR (\\r)
> - READ response: ET,BT,ET2,BT2,ambient,fan,heater (7 values)
> - Error format: ERR code message
> - Multiplexer: First command wins, 60-second timeout
> - Handshake disabled: Commands work immediately

## Introduction

This guide explains all the commands you can send to LibreRoaster during a coffee roasting session. Each command controls a specific aspect of your roaster, from reading temperatures to adjusting heat and fan speed.

**What you'll learn:**
- What each command does and when to use it
- How to format commands correctly
- How to read the responses
- What to do if you get an error

**How to use this guide:**
- Reference the Quick Reference table for fast lookup
- Read the detailed sections for in-depth understanding
- Follow the Roast Flow for a complete roasting session

---

## Quick Reference

| Command | Action | Example |
|---------|--------|---------|
| **READ** | Get current temperatures and outputs | `READ` |
| **OT1 XX** | Set heater power (0-100) | `OT1 50` |
| **IO3 XX** | Set fan speed (0-100) | `IO3 75` |
| **UP** | Increase heater by 5% | `UP` |
| **DOWN** | Decrease heater by 5% | `DOWN` |
| **START** | Begin roasting (start continuous output) | `START` |
| **STOP** | Emergency stop (disable all outputs) | `STOP` |

---

## Command Details

### READ — Get Current Readings

**What it does:**
Requests all current temperature readings and output percentages from the roaster.

**When to use:**
- Anytime you want to see current values
- At the start of a roast to establish a baseline
- Periodically during roasting to monitor progress

**What you send:**
```
READ
```

**What you get:**
```
185.2,192.3,-1,-1,24.5,45,75
```

**How to read the response:**

The response contains 7 values separated by commas:

| Position | Value | Meaning |
|----------|-------|---------|
| 1 | 185.2 | **ET** — Environment Temperature (your roaster's temperature sensor) |
| 2 | 192.3 | **BT** — Bean Temperature (the beans' temperature) |
| 3 | -1 | **ET2** — Extra channel 1 (not used, shows -1) |
| 4 | -1 | **BT2** — Extra channel 2 (not used, shows -1) |
| 5 | 24.5 | **Ambient** — Room temperature |
| 6 | 45 | **Fan** — Fan output percentage (0-100) |
| 7 | 75 | **Heater** — Heater output percentage (0-100) |

**Example in Artisan:**
Artisan displays ET and BT as real-time graphs on the scope. The other values appear in the device readout area.

---

### OT1 — Set Heater Power

**What it does:**
Controls the heater output as a percentage of maximum power.

**Range:** 0 (off) to 100 (full power)

**When to use:**
- During roasting to increase or decrease heat application
- Starting a roast: begin with 30-50% and adjust based on roast profile
- Reducing heat near the end of roasting (first crack)

**Examples:**
```
OT1 0     # Heater off
OT1 50    # 50% heater power
OT1 100   # Full heater power
```

**Tips:**
- Start conservatively — you can always increase heat
- Watch the BT (Bean Temperature) rate of rise
- Reduce heat gradually as you approach desired roast level
- Most roasts use 30-80% heater power

---

### IO3 — Set Fan Speed

**What it does:**
Controls the fan speed as a percentage of maximum.

**Range:** 0 (off) to 100 (full speed)

**When to use:**
- During roasting to control airflow
- Increasing fan cools the beans (more heat removal)
- Decreasing fan retains more heat

**Examples:**
```
IO3 0     # Fan off
IO3 50    # 50% fan speed
IO3 100   # Full fan speed
```

**Tips:**
- Higher fan speeds provide more cooling and faster heat removal
- Lower fan speeds retain more heat in the drum
- Adjust fan based on roast stage and desired development
- Common settings: 40-80% during most of the roast

---

### UP — Increase Heater

**What it does:**
Increases the heater output by exactly 5%.

**When to use:**
- Fine-tuning during roast when small adjustments are needed
- When you want precise control over heat changes
- Situations where you don't need exact percentages

**Example:**
```
UP
```

If heater is at 50%, after UP it becomes 55%.

**Tips:**
- Good for making subtle heat adjustments
- Easier than calculating exact OT1 values
- Watch the temperature response before making another adjustment

---

### DOWN — Decrease Heater

**What it does:**
Decreases the heater output by exactly 5%.

**When to use:**
- Reducing heat in small increments
- Fine-tuning near the end of roasting
- When you want precise but small reductions

**Example:**
```
DOWN
```

If heater is at 75%, after DOWN it becomes 70%.

**Tips:**
- Allows precise heat reduction
- Useful for controlling the roast trajectory
- Can be combined with fan adjustments for complex control

---

### START — Begin Roasting

**What it does:**
Enables continuous output streaming from the roaster to Artisan.

**When to use:**
- At the very beginning of your roast session
- After connecting to Artisan and verifying communication

**What happens:**
1. You send `START`
2. The roaster begins sending readings automatically
3. Artisan's scope starts updating in real-time
4. Temperature curves begin plotting

**Example:**
```
START
```

**After START:**
- The roaster sends readings at regular intervals
- Artisan displays real-time graphs
- You can send control commands (OT1, IO3, UP, DOWN)
- Send STOP when done

**Tips:**
- Send START once at the beginning
- Keep Artisan connected throughout the roast
- The connection stays active while you're sending commands

---

### STOP — Emergency Stop

**What it does:**
Immediately disables the heater and fan outputs.

**When to use:**
- **Emergency situations only**
- When something goes wrong during roasting
- To quickly shut down all outputs

**What happens:**
1. Heater output goes to 0%
2. Fan output goes to 0%
3. Temperature readings continue
4. Roaster enters safe state

**After STOP:**
- You must send START again to resume output streaming
- Control commands still work (you can set OT1/IO3 and send START again)

**Warning:**
This is for emergencies only. Normal shutdown uses OT1 0 and IO3 0.

---

## Understanding Responses

### READ Response Breakdown

The `READ` command returns 7 values:

```
ET,BT,ET2,BT2,ambient,fan,heater
```

**Complete example:**
```
185.2,192.3,-1,-1,24.5,45,75
```

| Field | Value | Description |
|-------|-------|-------------|
| ET | 185.2 | Environment Temperature — sensor near the beans |
| BT | 192.3 | Bean Temperature — temperature of the beans |
| ET2 | -1 | Extra channel 1 — not connected, shows -1 |
| BT2 | -1 | Extra channel 2 — not connected, shows -1 |
| ambient | 24.5 | Room temperature |
| fan | 45 | Current fan output % |
| heater | 75 | Current heater output % |

**In Artisan:**
- ET and BT appear as colored lines on the scope graph
- The other values show in Artisan's device information panel

---

## Error Messages

If something is wrong with your command, you'll get an error response:

```
ERR code message
```

### Common Errors

| Error | Meaning | What to Do |
|-------|---------|------------|
| `ERR empty_command` | You sent nothing or just whitespace | Type a command like `READ` |
| `ERR unknown_command` | Command not recognized | Check spelling: `READ`, `OT1`, `IO3`, `UP`, `DOWN`, `START`, `STOP` |
| `ERR invalid_value` | Number is not valid | Use 0-100 for OT1 and IO3 |
| `ERR out_of_range` | Number is too large | Use values between 0 and 100 |

### Error Recovery

1. **Read the error message** — it tells you what went wrong
2. **Fix the command** — correct the spelling or value
3. **Send again** — try the corrected command

**Examples:**
```
# Wrong: OT1 150 (too high)
ERR invalid_value out_of_range

# Correct: OT1 50
(no error - command accepted)

# Wrong: REDA (typo)
ERR unknown_command unknown_command

# Correct: READ
185.2,192.3,-1,-1,24.5,45,75
```

---

## Roast Flow

Follow this sequence during a typical roasting session:

### Step 1: Connect to Artisan
- Follow the [Artisan Connection Guide](ARTISAN_CONNECTION.md)
- Verify the port and baud rate (115200)
- Click Connect in Artisan

### Step 2: Send START
```
START
```
- Enables continuous readings
- Artisan's scope begins plotting

### Step 3: Monitor Temperatures
- Watch ET and BT curves develop
- Note the rate of rise (RoR)
- BT should follow ET with some delay

### Step 4: Control Heat and Fan
- Adjust heater: `OT1 XX` or `UP`/`DOWN`
- Adjust fan: `IO3 XX`
- Make small adjustments, wait for response
- Watch temperature curves

### Step 5: End the Roast
- Send `STOP` for emergency stop
- Or reduce heater/fan to 0 and let Artisan finish

---

## Tips and Best Practices

### General Tips

1. **Watch trends, not just values**
   - The RoR (rate of rise) matters more than absolute temperature
   - A rising BT indicates active roasting
   - Flat BT means no heat is being absorbed

2. **Make small adjustments**
   - Heat changes take time to affect bean temperature
   - Wait 15-30 seconds between adjustments
   - Patience leads to better control

3. **Keep hands ready for STOP**
   - Always know where the emergency stop is
   - Don't leave the roaster unattended during roasting

4. **Use Artisan's tools**
   - The scope shows the complete picture
   - Use events to mark key moments (first crack, etc.)
   - Review the roast profile after completion

### Command Tips

- **UP/DOWN for fine-tuning** — Small changes matter
- **OT1 for major adjustments** — Large heat changes
- **IO3 for airflow control** — Complement your heat adjustments
- **READ anytime** — Verify current state

---

## Reference

| Setting | Value |
|---------|-------|
| Baud Rate | 115200 |
| Command Format | ASCII terminated with CR (\\r) |
| READ Response | 7 comma-separated values |
| Error Format | ERR code message |
| Heater Range | 0-100 (%) |
| Fan Range | 0-100 (%) |
| Step Size (UP/DOWN) | 5% |

---

*See also: [PROTOCOL.md](PROTOCOL.md) for technical details*
