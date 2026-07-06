#!/usr/bin/env node
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { createAppContext } from "./context.js";
import { createServer } from "./tools.js";

const context = createAppContext();
const server = createServer(context);
const transport = new StdioServerTransport();

await server.connect(transport);
