# mqnutd
*MQTT connector for NUT protocol*

Takes network-attached UPS status via NUT and translates it to MQTT, or receives such MQTT updates and performs system calls to safely power off.

---

I got tired of frequent power outages leaving me to shut down everything manually. `mqnutd` leaves that task to:

1. 🔌 UPS to power things for a few minutes,
2. 🦟 MQTT to transfer statuses of UPSs (*),
3. 🖥️ `mqnutd` to intercept UPS status and power off devices as planned

> (*): A feature of MQTT called 'Last Will Message' helps to prevent missed MQTT updates by firing events if, say, the network dies during an outage. `mqnutd` checks for this condition and will attempt to stop a machine before UPS drains itself to death.


