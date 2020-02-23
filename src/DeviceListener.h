#ifndef LAYKEYMAP_DEVICELISTENER_H
#define LAYKEYMAP_DEVICELISTENER_H

#include "poll/Subscriber.h"
#include <string>

class DeviceListener : public Subscriber {
public:

	DeviceListener() : Subscriber(std::string("device-listener")) {

	}

	virtual void on_error(Poll &p, int event) {
	}

	virtual void on_input(Poll &p) {

	}

	virtual void on_output(Poll &p) {
	}

	virtual ~DeviceListener() {

	}
};

#endif //LAYKEYMAP_DEVICELISTENER_H
