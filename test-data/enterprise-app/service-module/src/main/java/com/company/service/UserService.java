package com.company.service;

import com.company.core.Utils;
import org.springframework.stereotype.Service;

@Service
public class UserService {
    public void createUser(String username) {
        if (Utils.isEmpty(username)) {
            throw new IllegalArgumentException("Username cannot be empty");
        }
        System.out.println("Creating user: " + username);
    }
}
