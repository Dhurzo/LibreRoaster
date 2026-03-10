# Hardware Validation Methodology

## Command Latency
Latency is measured as the time delta (in microseconds) between the arrival/dequeue of an Artisan command and the completion of its respective handler. This measures the internal firmware processing overhead and ensures the control loop remains responsive.

## Thermal Envelope
The thermal envelope represents the stability of the heating control. It is calculated as the absolute difference between the actual Bean Temperature (BT) and the target temperature during a stable heating phase (post-preheat, during a soak or constant rate of rise).

## Safety Metrics
- **Watchdog Fails**: Any consecutive watchdog failure is considered a critical safety breach.
- **LEDC Guard Timeouts**: Occasional timeouts are acceptable if the system recovers, but excessive timeouts indicate potential hardware or scheduling contention.
