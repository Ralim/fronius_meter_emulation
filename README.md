# Fronius Smart Meter IP Emulation

TL;DR I'm not installing multiple vendors smart meters because of vendor locking.

This packset is setup to read from a Shelly 3EM over TCP Modbus, and then provide those readings to a Fronius solar inverter.
This setup will **NEVER** be perfect. You will not be able to ever use this for 0 export control.
This is used as "near enough" control for situations where you _can_ export to the grid, but pricing may be non optimal.
Such as in Australia when on wholesale pricing and during the middle of the day when export prices go negative.

All this code does is transfer the power readings over.


### Emulated meter

The emulated meter does not implement writing.


## Kudos

https://www.photovoltaikforum.com/thread/224214-gen24-smart-meter-modbus-tcp-emulation-mit-esp32/
