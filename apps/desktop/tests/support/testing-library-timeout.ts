import { configure } from "@testing-library/react";

// The page-level tests run alongside other jsdom workers and native builds.
// Keep condition-based waits inside the 60 s test budget when rendering stalls under load.
configure({ asyncUtilTimeout: 5000 });
