#!/usr/bin/env bash

set -e

sh ./scripts/build.sh

################ SYSTEMD ############################
SYSTEMD_CONF_PATH="/lib/systemd/system/laykeymap.service"
echo "Installing systemd configuration file to ${SYSTEMD_CONF_PATH}"
{
  sudo rm ${SYSTEMD_CONF_PATH}
  sudo tee -a ${SYSTEMD_CONF_PATH} <<EOF
[Unit]
Description=Laykeymap keyboard mapper daemon

[Service]
Type=simple
User=root
ExecStart=/usr/bin/laykeymap grab /etc/laykeymap

[Install]
WantedBy=multi-user.target
EOF
} >/dev/null

################ JSON CONFIG #########################
KEYMAP_CONF_PATH="/etc/laykeymap"
echo "Installing systemd configuration file to ${KEYMAP_CONF_PATH}"
{
  sudo rm ${KEYMAP_CONF_PATH}
  sudo tee -a ${KEYMAP_CONF_PATH} <<EOF
{
  "map": {
    "KEY_SYSRQ": "KEY_RIGHTCTRL"
  },
  "layers": {
    "KEY_RIGHTALT": {
      "KEY_L": "KEY_RIGHT",
      "KEY_I": "KEY_UP",
      "KEY_J": "KEY_LEFT",
      "KEY_K": "KEY_DOWN",
      "KEY_U": "KEY_PAGEUP",
      "KEY_O": "KEY_PAGEDOWN"
    }
  },
  "keyboardNames": [ "AT Translated Set 2 keyboard" ]
}
EOF
} >/dev/null

################ EXECUTABLE #######################

echo "Installing executable to /usr/bin"
sudo cp build/src/laykeymap /usr/bin

echo
echo "   Laykeymap installed succesfully!"
echo "   Further steps:"
echo "   1) Edit /etc/laykeymap to configure the key mapping"
echo "   2) Test the configuration by laykeymap grab /etc/laykeymap"
echo "   3) If you're satisfied run the mapper as backgorund process"
echo "     sudo systemctl enable laykeymap && sudo systemctl start laykeymap"
