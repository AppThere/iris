// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

// Integration tests for iris-aif.
// Each test file in tests/fixtures/ must have a corresponding test here.

// NOTE: AifError::PermissionRevoked is not testable on the desktop CI host.
// It requires a FileAccessToken whose check_permission() returns Revoked.
// A mock injection point in loki-file-access is needed for this — deferred.
// TODO(iris): SPEC.md §10 — add PermissionRevoked test once loki-file-access mock API exists.
