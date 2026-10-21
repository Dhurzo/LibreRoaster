// MAX31856 thermocouple amplifier emulator for Wokwi.
//
// Implements the subset of the MAX31856 SPI protocol used by LibreRoaster:
//   - 8-bit register address, A7 = 1 for writes, A7 = 0 for reads
//   - 6-byte conversion block at 0x0A..0x0F:
//     CJTH, CJTL, LTCBH, LTCBM, LTCBL, SR
//   - one-shot conversion on CR0 bit 6 (0x40), 50 Hz filter bit 0
//   - SPI mode 1 (CPOL = 0, CPHA = 1)
//
// Temperature encoding matches the datasheet: the 19-bit result is
// left-justified in the 24-bit LTC register (bits 23:5), 1 LSB = 1/128 C.
// The cold-junction result is 14-bit two's complement, left-justified in
// the 16-bit CJTH:CJTL pair (bits 15:2), 1 LSB = 1/64 C.
//
// Attributes (see max31856.chip.json): temperature, coldJunction, fault.
// Two instances are used: BT on CS = GPIO4 and ET on CS = GPIO3.

#include "wokwi-api.h"

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define REG_CR0 0x00
#define REG_CR1 0x01
#define REG_MASK 0x02
#define REG_CJTH 0x0A
#define REG_CJTL 0x0B
#define REG_LTCBH 0x0C
#define REG_LTCBM 0x0D
#define REG_LTCBL 0x0E
#define REG_SR 0x0F

#define REG_COUNT 16

typedef struct {
  spi_dev_t spi;
  pin_t cs;
  uint8_t regs[REG_COUNT];
  uint8_t byte;
  uint8_t address;
  uint8_t index;
  bool is_write;
  uint32_t temperature_attr;
  uint32_t cold_junction_attr;
  uint32_t fault_attr;
} chip_state_t;

static int32_t round_to_i32(float value) {
  return (int32_t)(value >= 0.0f ? value + 0.5f : value - 0.5f);
}

static void update_conversion(chip_state_t *chip) {
  float temperature = attr_read_float(chip->temperature_attr);
  int32_t raw19 = round_to_i32(temperature * 128.0f);
  uint32_t raw24 = ((uint32_t)raw19 << 5) & 0xFFFFFFu;

  chip->regs[REG_LTCBH] = (uint8_t)((raw24 >> 16) & 0xFFu);
  chip->regs[REG_LTCBM] = (uint8_t)((raw24 >> 8) & 0xFFu);
  chip->regs[REG_LTCBL] = (uint8_t)(raw24 & 0xFFu);

  float cold_junction = attr_read_float(chip->cold_junction_attr);
  int32_t cj14 = round_to_i32(cold_junction * 64.0f);
  uint16_t cj16 = (uint16_t)((uint16_t)(int16_t)cj14 << 2);

  chip->regs[REG_CJTH] = (uint8_t)((cj16 >> 8) & 0xFFu);
  chip->regs[REG_CJTL] = (uint8_t)(cj16 & 0xFFu);
  chip->regs[REG_SR] = (uint8_t)attr_read(chip->fault_attr);
}

static uint8_t read_register(chip_state_t *chip, uint8_t address) {
  uint8_t reg = address & 0x0Fu;
  if (reg == REG_SR) {
    return (uint8_t)attr_read(chip->fault_attr);
  }
  return chip->regs[reg];
}

static void spi_done(void *user_data, uint8_t *buffer, uint32_t count) {
  chip_state_t *chip = user_data;

  if (count > 0) {
    uint8_t received = buffer[0];

    if (chip->index == 0) {
      chip->is_write = (received & 0x80u) != 0;
      chip->address = received & 0x7Fu;
      chip->index = 1;
      chip->byte = chip->is_write ? 0x00u : read_register(chip, chip->address);
    } else if (chip->is_write) {
      if (chip->index == 1) {
        uint8_t reg = chip->address & 0x0Fu;
        chip->regs[reg] = received;
        if (reg == REG_CR0 && (received & 0x40u) != 0) {
          update_conversion(chip);
        }
      }
      chip->index++;
      chip->byte = 0x00u;
    } else {
      chip->index++;
      chip->byte = read_register(chip, (uint8_t)(chip->address + chip->index - 1u));
    }
  }

  if (pin_read(chip->cs) == LOW) {
    spi_start(chip->spi, &chip->byte, 1);
  }
}

static void cs_change(void *user_data, pin_t pin, uint32_t value) {
  chip_state_t *chip = user_data;

  if (value == LOW) {
    chip->index = 0;
    chip->is_write = false;
    chip->address = 0;
    chip->byte = 0x00u;
    spi_start(chip->spi, &chip->byte, 1);
  } else {
    spi_stop(chip->spi);
  }
}

void chip_init(void) {
  chip_state_t *chip = calloc(1, sizeof(chip_state_t));
  if (chip == NULL) {
    return;
  }

  chip->regs[REG_CR0] = 0x11u;  // CMODE = 0, OCFAULT = 01, FILT50 = 1
  chip->regs[REG_CR1] = 0x03u;  // one sample average, Type K

  chip->temperature_attr = attr_init_float("temperature", 25.0f);
  chip->cold_junction_attr = attr_init_float("coldJunction", 25.0f);
  chip->fault_attr = attr_init("fault", 0);
  update_conversion(chip);

  chip->cs = pin_init("CS", INPUT);
  const pin_watch_config_t watch = {
      .edge = BOTH,
      .pin_change = cs_change,
      .user_data = chip,
  };
  pin_watch(chip->cs, &watch);

  const spi_config_t spi_config = {
      .sck = pin_init("SCK", INPUT),
      .mosi = pin_init("MOSI", INPUT),
      .miso = pin_init("MISO", INPUT),
      .mode = 1,
      .done = spi_done,
      .user_data = chip,
  };
  chip->spi = spi_init(&spi_config);
}
