//
// Created by Samvel Abrahamyan on 2019-05-30.
//

#ifndef DISTFS_ALARM_H
#define DISTFS_ALARM_H

#include "Subscriber.h"
#include <functional>

/**
 * Since BSD and MacOS don't have the timerfd_create syscall,
 * we cannot poll on alarm file descriptor.
 * That's why I choose to busy wait for alarms to expire, using poll timeout
 */

uint64_t current_time_millis();

class Alarm {
private:
	std::function<void()> callback;
private:
	uint64_t millis;

public:
	void on_timeout() const;

	Alarm(uint64_t millis, std::function<void()> callback);

	uint64_t get_timeout_time();

	~Alarm() = default;

};

#endif //DISTFS_ALARM_H
