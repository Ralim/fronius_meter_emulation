# Fronius Smart Meter IP Emulation

TL;DR I'm not installing multiple vendors smart meters because of vendor locking.

This software is setup to read from a Shelly 3EM over TCP Modbus, and then provide those readings to a Fronius solar inverter.
This setup will **NEVER** be perfect. You will not be able to ever use this for 0 export control.
This is used as "near enough" control for situations where you _can_ export to the grid, but pricing may be non optimal.
Such as in Australia when on wholesale pricing and during the middle of the day when export prices go negative.
All this code does is transfer the power readings over.

To enhance this slightly further, biasing values can be read from home assistant.
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

### Home Assistant

The Home Assistant controls are read over the API from home assitant at approximately 1Hz.
To aid in control, there are two controls supported; which are added as virtual export and virtual import.
This means if you have a virtual export of 1000W and a virtual import of 400W, a net shift of 600W of export is added to the raw meter
reading before its reported to the virtual meter.


### The Emulated meter

The emulated meter does not implement writing.
The software has code to handle most of the readings published by the Fronius smart meter; but in testing its been found the inverter only looks at the net wattage values anyway.
So the code doesnt bother with the rest and instead just implements those to keep latency down


## Kudos

https://www.photovoltaikforum.com/thread/224214-gen24-smart-meter-modbus-tcp-emulation-mit-esp32/
