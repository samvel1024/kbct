#include <iostream>
#include "KeyListener.h"
#include "KeyMapper.h"
#include "UInput.h"
#include "poll/Poll.h"
#include "poll/KillReceiver.h"
#include "DeviceListener.h"
#include "KeyboardGrabManager.h"

int main(int argc, char **argv) {
	if (argc != 3) {
		std::cerr << "Usage: simkey <device_path> <json_path>";
		return 1;
	}
	std::string device = argv[1];
	std::string json = argv[2];
	KeyboardGrabManager manager(json);
	manager.add_listener(device);
	manager.listen();
}