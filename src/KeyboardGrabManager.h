#ifndef LAYKEYMAP_KEYBOARDGRABMANAGER_H
#define LAYKEYMAP_KEYBOARDGRABMANAGER_H

#include "poll/Poll.h"
#include "KeyListener.h"
#include "UInput.h"
#include "KeyMapper.h"
#include <unordered_map>

class KeyboardGrabManager {

private:
	UInput uinput;
	Poll poll;
	std::unordered_map<std::string, KeyListener *> listeners;
	KeyMapper mapper;

public:

	KeyboardGrabManager(std::string &json) : mapper(KeyMapper::configure_from_json(json, [&](auto p, auto l) {
		uinput.consume(p, l);
	})) {
	}

	void listen() {
		poll.loop();
	}

	bool is_grabbed(std::string &device) {
		return listeners.find(device) != listeners.end();
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

	void add_listener(std::string &device) {
		if (is_grabbed(device))
			throw std::exception();
		auto sp = std::make_shared<KeyListener>(device, [=]() mutable {
			remove_listener(device);
		}, [&](auto v) {
			mapper.on_keystroke(v);
		});
		poll.subscribe(sp);
		listeners[device] = sp.get();
		std::cout << "Grabbed device " << device << std::endl;
	}
};

#endif //LAYKEYMAP_KEYBOARDGRABMANAGER_H
