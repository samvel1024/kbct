//
// Created by sme on 23/02/2020.
//

#ifndef LAYKEYMAP_KEYMAPCONFIG_H
#define LAYKEYMAP_KEYMAPCONFIG_H

#include <unordered_map>
#include <fstream>
#include <iostream>
#include <nlohmann/json.hpp>
#include "KeyMapper.h"

struct KeymapConfig {

	using json = nlohmann::json;

	std::unordered_map<std::string, std::string> map;
	std::unordered_map<std::string, std::unordered_map<std::string, std::string>> layers;
	std::vector<std::string> keyboard_names;

	static KeymapConfig parse_from_json_file(std::string &json_file) {
		std::ifstream is(json_file, std::ifstream::in);
		json json_conf = json::parse(is);

		if (!json_conf.contains("map"))
			json_conf["map"] = {};
		if (!json_conf.contains("layers"))
			json_conf["layers"] = {};


		if (json_conf.size() > 3) {
			std::cerr << "JSON has more than required keys (map, layers, keyboardNames). See an example usage" << std::endl;
			throw std::exception();
		}

		auto names = json_conf["keyboardNames"].get<std::vector<std::string>>();
		if (names.empty()) {
			std::cerr << "Got empty set of keyboads" << std::endl;
			throw std::exception();
		}
		return {
				json_conf["map"].get<std::unordered_map<std::string, std::string >>(),
				json_conf["layers"].get<std::unordered_map<std::string, std::unordered_map<std::string, std::string >>>(),
				names
		};
	}
};


#endif //LAYKEYMAP_KEYMAPCONFIG_H
