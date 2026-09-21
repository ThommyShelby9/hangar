# Modules PawnIO

`IntelMSR.bin` : module officiel signé de [PawnIO.Modules](https://github.com/namazso/PawnIO.Modules),
version **0.2.11** (`release_0_2_11.zip`), licence LGPL 2.1. Le pilote PawnIO refuse tout module non signé.

`hangar-sensors.exe` l'utilise pour lire, en lecture seule, les registres de température
(`IA32_THERM_STATUS`, `IA32_PACKAGE_THERM_STATUS`, `IA32_TEMPERATURE_TARGET`) et d'énergie
(`MSR_RAPL_POWER_UNIT`, `MSR_PKG_ENERGY_STATUS`) du processeur.
