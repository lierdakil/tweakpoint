## services\.tweakpoint\.enable

Whether to enable tweakpoint service\.



*Type:*
boolean



*Default:*
` false `



*Example:*
` true `



## services\.tweakpoint\.extraPkgs



Extra packages to include into the search path



*Type:*
list of package



*Default:*
` [ ] `



*Example:*

```
[
  <derivation xinput-1.6.4>
]
```



## services\.tweakpoint\.logLevel



Log level



*Type:*
one of “trace”, “debug”, “info”, “warn”, “error”



*Default:*
` "error" `



*Example:*
` "info" `



## services\.tweakpoint\.postScript



Script to run after tweakpoint starts



*Type:*
null or strings concatenated with “\\n”



*Default:*
` null `



*Example:*

```
''
  #!/usr/bin/env bash
  while ! xinput list-props pointer:tweakpoint; do sleep 0.1s; done
  xinput set-prop pointer:tweakpoint 'libinput Accel Profile Enabled' 0 1 0
  xinput set-prop pointer:tweakpoint 'libinput Accel Speed' 0.55
''
```



## services\.tweakpoint\.settings\.axis_map\.regular



Map axis to other axis, when scroll mode disabled



*Type:*
attribute set of (submodule)



*Default:*
` { } `



*Example:*

```
{
  REL_WHEEL = {
    axis = "REL_RESERVED";
  };
}
```



## services\.tweakpoint\.settings\.axis_map\.regular\.\<name>\.axis



Relative axis name



*Type:*
one of “REL_X”, “REL_Y”, “REL_Z”, “REL_RX”, “REL_RY”, “REL_RZ”, “REL_HWHEEL”, “REL_DIAL”, “REL_WHEEL”, “REL_MISC”, “REL_RESERVED”, “REL_WHEEL_HI_RES”, “REL_HWHEEL_HI_RES”



*Example:*
` "REL_X" `



## services\.tweakpoint\.settings\.axis_map\.regular\.\<name>\.factor



Factor between original axis and new axis movement



*Type:*
floating point number



*Default:*
` 1.0 `



*Example:*
` 0.1 `



## services\.tweakpoint\.settings\.axis_map\.scroll



Map axis to other axis, when scroll mode enabled



*Type:*
attribute set of (submodule)



*Default:*
` { } `



*Example:*

```
{
  REL_X = {
    axis = "REL_HWHEEL_HI_RES";
    factor = 10.0;
  };
  REL_Y = {
    axis = "REL_WHEEL_HI_RES";
    factor = -10.0;
  };
}
```



## services\.tweakpoint\.settings\.axis_map\.scroll\.\<name>\.axis



Relative axis name



*Type:*
one of “REL_X”, “REL_Y”, “REL_Z”, “REL_RX”, “REL_RY”, “REL_RZ”, “REL_HWHEEL”, “REL_DIAL”, “REL_WHEEL”, “REL_MISC”, “REL_RESERVED”, “REL_WHEEL_HI_RES”, “REL_HWHEEL_HI_RES”



*Example:*
` "REL_X" `



## services\.tweakpoint\.settings\.axis_map\.scroll\.\<name>\.factor



Factor between original axis and new axis movement



*Type:*
floating point number



*Default:*
` 1.0 `



*Example:*
` 0.1 `



## services\.tweakpoint\.settings\.btn_map



Map buttons to other buttons



*Type:*
attribute set of (Evdev key code name, e\.g\. “BTN_LEFT” or “KEY_A”)



*Default:*
` { } `



*Example:*

```
{
  BTN_MIDDLE = "BTN_LEFT";
  BTN_SIDE = "BTN_TASK";
}
```



## services\.tweakpoint\.settings\.bus



Reported bus type of the virtual pointer device



*Type:*
one of “BUS_PCI”, “BUS_ISAPNP”, “BUS_USB”, “BUS_HIL”, “BUS_BLUETOOTH”, “BUS_VIRTUAL”, “BUS_ISA”, “BUS_I8042”, “BUS_XTKBD”, “BUS_RS232”, “BUS_GAMEPORT”, “BUS_PARPORT”, “BUS_AMIGA”, “BUS_ADB”, “BUS_I2C”, “BUS_HOST”, “BUS_GSC”, “BUS_ATARI”, “BUS_SPI”, “BUS_RMI”, “BUS_CEC”, “BUS_INTEL_ISHTP”



*Default:*
` "BUS_USB" `



## services\.tweakpoint\.settings\.device



Path to the input event device file



*Type:*
absolute path



*Example:*
` "/dev/input/by-id/usb-Foo-Bar" `



## services\.tweakpoint\.settings\.hi_res_enabled



Enable high-resolution wheel events?



*Type:*
boolean



*Default:*
` true `



## services\.tweakpoint\.settings\.meta\.chord



Action when other button is pressed together with the meta button



*Type:*
attribute set of (one of “None”, “ToggleScroll” or ({ Button = key_code }) or ({ ToggleLock = \[ key_code ] }) or ({ Gesture = { “gesture_key” = action } }, where gesture_key is a sequence of U, D, L, R))



*Default:*
` { } `



*Example:*

```
{
  BTN_LEFT = "ToggleScroll";
}
```



## services\.tweakpoint\.settings\.meta\.click



Click action



*Type:*
one of “None”, “ToggleScroll” or ({ Button = key_code }) or ({ ToggleLock = \[ key_code ] }) or ({ Gesture = { “gesture_key” = action } }, where gesture_key is a sequence of U, D, L, R)



*Default:*
` "None" `



## services\.tweakpoint\.settings\.meta\.hold



Hold action



*Type:*
one of “None”, “ToggleScroll” or ({ Button = key_code }) or ({ ToggleLock = \[ key_code ] }) or ({ Gesture = { “gesture_key” = action } }, where gesture_key is a sequence of U, D, L, R)



*Default:*
` "None" `



## services\.tweakpoint\.settings\.meta\.hold_time



Hold timeout, with suffix s/ms/\&c



*Type:*
string



*Default:*
` "250ms" `



*Example:*
` "500ms" `



## services\.tweakpoint\.settings\.meta\.key



Meta key



*Type:*
Evdev key code name, e\.g\. “BTN_LEFT” or “KEY_A”



*Example:*
` "BTN_MIDDLE" `



## services\.tweakpoint\.settings\.meta\.move



Move action; action performed when pointer is moved while meta button is pressed



*Type:*
one of “None”, “ToggleScroll” or ({ Button = key_code }) or ({ ToggleLock = \[ key_code ] }) or ({ Gesture = { “gesture_key” = action } }, where gesture_key is a sequence of U, D, L, R)



*Default:*
` "None" `



## services\.tweakpoint\.settings\.name



Human-readable name of the virtual pointer device



*Type:*
string



*Default:*
` "tweakpoint" `



*Example:*
` "tweakpoint" `



## services\.tweakpoint\.settings\.product_id



Reported product_id of the virtual pointer device



*Type:*
16 bit unsigned integer; between 0 and 65535 (both inclusive)



*Default:*
` 1 `



## services\.tweakpoint\.settings\.product_version



Reported product version of the virtual pointer device



*Type:*
16 bit unsigned integer; between 0 and 65535 (both inclusive)



*Default:*
` 1 `



## services\.tweakpoint\.settings\.socket_path



Path to the control socket



*Type:*
null or absolute path



*Default:*
` null `



*Example:*
` "/tmp/tweakpoint.sock" `



## services\.tweakpoint\.settings\.vendor_id



Reported vendor_id of the virtual pointer device



*Type:*
16 bit unsigned integer; between 0 and 65535 (both inclusive)



*Default:*
` 1 `



## services\.tweakpoint\.settingsFile



Path to the config file\. Overrides settings if set\.
Note that it won’t be checked for correctness



*Type:*
absolute path or package



*Default:*
` <derivation tweakpoint.toml> `


