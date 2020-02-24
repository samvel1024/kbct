#ifndef LAYKEYMAP_KEYBOARDGRABMANAGER_H
#define LAYKEYMAP_KEYBOARDGRABMANAGER_H

#include "poll/Poll.h"
#include "KeyListener.h"
#include "UInput.h"
#include "KeyMapper.h"
#include <unordered_map>
#include <filesystem>
#include <libevdev-1.0/libevdev/libevdev.h>
#include <type_traits>

class KeyboardGrabManager {
public:
	struct DeviceDescriptor {
		std::string driver;
		std::string name;

		friend std::ostream &operator<<(std::ostream &out, DeviceDescriptor const &dd) {
			return out << "Device( name='" << dd.name << "' driver='" << dd.driver << "')";
		}
	};

private:
	UInput uinput;
	Poll poll;
	std::unordered_map<std::string, KeyListener *> listeners;
	KeyMapper mapper;
	std::vector<std::string> keyboard_names;

public:

	KeyboardGrabManager(KeymapConfig conf) : mapper(conf, [&](auto p, auto l) {
		uinput.consume(p, l);
	}), keyboard_names(conf.keyboard_names) {
		libevdev_set_log_function(NULL, NULL);
		update_grabbed_keyboards();
	}

	void listen() {
		auto dsl = std::make_shared<DeviceStateListener>([&]() {
			update_grabbed_keyboards();
		});
		poll.subscribe(dsl);

		auto killer = std::make_shared<KillReceiver>();
		poll.subscribe(killer);

		poll.loop();
	}

	bool is_grabbed(const std::string &device) {
		return listeners.find(device) != listeners.end();
	}

	void update_grabbed_keyboards() {
		auto devs = get_keyboard_devices();
		for (auto const &d: devs) {
			for (auto const &s : keyboard_names) {
				if (s.compare(d.name) == 0 && !is_grabbed(d.driver)) {
					std::cout << "Matched keyboard " << d << std::endl;
					add_listener(d.driver);
				}
			}
		}
	}

	static std::vector<DeviceDescriptor> get_keyboard_devices() {
		namespace fs = std::filesystem;
		std::vector<DeviceDescriptor> devices;

		for (const auto &entry: fs::directory_iterator("/dev/input")) {
			if (entry.is_character_file()) {
				std::string path = fs::absolute(entry.path()).string();
				struct libevdev *dev;
				int fd = open(path.c_str(), O_RDWR);
				if (fd < 0) {
					std::cerr << "Could not open " << path << " ERROR " << std::strerror(errno) << std::endl;
					continue;
				}

				if (libevdev_new_from_fd(fd, &dev) < 0) {
					continue;
				}

				// Check if it's a real keyboard (can be numpad only as well)
				if (!libevdev_has_event_code(dev, EV_KEY, KEY_1)) {
					continue;
				}
				char const *c_name = libevdev_get_name(dev);
				std::string name = c_name == NULL ? "" : std::string(c_name);
				devices.push_back({.driver = path, .name = name});

				libevdev_free(dev);
			}
		}
		return devices;
	}

	void remove_listener(std::string &device) {
		if (!is_grabbed(device)) {
			throw std::exception();
		}
		KeyListener *listener = listeners[device];
		poll.unsubscribe(*listener);
		listeners.erase(listeners.find(device));
		std::cout << "Ungrabbed device " << device << std::endl;
	}

	void add_listener(const std::string &device) {
		if (is_grabbed(device))
			throw std::exception();
		std::string dev = device;
		auto sp = std::make_shared<KeyListener>(dev, [=]() mutable {
			remove_listener(dev);
		}, [&](auto v) {
			mapper.on_keystroke(v);
		});
		poll.subscribe(sp);
		listeners[device] = sp.get();
		std::cout << "Grabbed device " << device << std::endl;
	}
};

#endif //LAYKEYMAP_KEYBOARDGRABMANAGER_H
