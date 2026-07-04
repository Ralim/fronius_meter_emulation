# Fronius Smart Meter IP Emulation

TL;DR I'm not installing multiple vendors smart meters because of vendor locking.

This software is setup to read from a Shelly 3EM over TCP Modbus, and then provide those readings to a Fronius solar inverter.
This setup will **NEVER** be perfect. You will **not** be able to ever use this for 0 export control. (There will always be some small errors as Shelly meters cant read faster than 1Hz).
This is used as "near enough" control for situations where you _can_ export to the grid, but pricing may be non optimal.
Or for situations where you just want to have the readings available to the inverter for its app / charts etc.
Such as in Australia when on wholesale pricing and during the middle of the day when export prices go negative.
All this code does is transfer the power readings over between the Shelly Modbus and the Fronius Modbus TCP interface.

To enhance this slightly further, biasing values can be read from home assistant (or another http server).
These will be merged in with the "real" readings, to allow software to drive the inverter to regulate around a different setpoint.
This is useful as normally you would set the inverter to some limit X of the maximum grid export amount, and it will internally use a control loop
to try and regulate its output to keep at that setpoint of export.
By using these inputs you can then shift this regulation point to where is desired.
This can be useful when having other controlled loads that are also export power aware.

## Usage

This software is best run as a docker container on a device that has a reliable network connection to all involved devices (i.e avoid WiFi if you can).

### The source meter

At the moment the only source meter is the Shelly 3EM, more can be added if desired.
This meter is read via modbus, as this provides the simplest means of capturing the measurements.

The Shelly modbus address and port must be specified via the `SHELLY_MODBUS` env var.

The code priorities updating the live power reading at 2Hz. Shelly Devices only update at 1Hz but this is done to reduce latency.
Other sensors (per phase readings, Wh totals) are updated much slower as the Inverter doesnt (yet?) appear to use them, and the Shelly doesnt like getting hammered for modbus messages.
The code defaults to updating these every 60 seconds. You can increase the speed by setting the `SLOW_METER_READ_INTERVAL_S` env var to a lower value. (Such as 5 for 5seconds).


### Home Assistant

The Home Assistant controls are read over the API from home assitant at approximately 1Hz.
To aid in control, there are two controls supported; which are added as virtual export and virtual import.
This means if you have a virtual export of 1000W and a virtual import of 400W, a net shift of 600W of export is added to the raw meter
reading before its reported to the virtual meter.

The Home Assistant integration can be configured via the `HA_URL`, `HA_TOKEN`, `HA_EXTRA_IMPORT` and `HA_EXTRA_EXPORT` env vars.

### The Emulated meter

The emulated meter does not implement writing.
The software has code to handle most of the readings published by the Fronius smart meter; but in testing its been found the inverter only looks at the net wattage values anyway.
So the code doesnt bother with the rest and instead just implements those to keep latency down

By default the modbus socket is bound on `0.0.0.0:1502`, but this can be overridden using the `FRONIUS_MODBUS_BIND` env var.
Port 1502 is used as its available in most Fronius firmware's, but you can override it to use a different port if needed.


## Configuration

The code is entirely configured by environment variables, to make running in docker easier without having to map volume mounts etc.

```bash
HA_EXTRA_IMPORT # Home Assistant sensor that is added to import readings
HA_EXTRA_EXPORT # Home Assistant sensor that is added to export readings
SHELLY_MODBUS # Modbus address of the Shelly 3EM meter
SLOW_METER_READ_INTERVAL_S # Interval in seconds between slow meter reads
HA_SMOOTH # Home Assistant smoothing enable disable (true/false)
HA_URL # Home Assistant URL to connect to
HA_TOKEN # Home Assistant token for authentication
FRONIUS_MODBUS_BIND # IP:port to bind the modbus socket to for emulating the Fronius meter
```
## Kudos

https://www.photovoltaikforum.com/thread/224214-gen24-smart-meter-modbus-tcp-emulation-mit-esp32/
