CREATE TABLE `assistant_messages` (
	`uuid` text PRIMARY KEY NOT NULL,
	`cost_usd` real NOT NULL,
	`duration_ms` integer NOT NULL,
	`message` text NOT NULL,
	`is_api_error_message` integer DEFAULT false NOT NULL,
	`timestamp` integer NOT NULL, `model` text DEFAULT '' NOT NULL,
	FOREIGN KEY (`uuid`) REFERENCES `base_messages`(`uuid`) ON UPDATE no action ON DELETE no action
);
CREATE TABLE `base_messages` (
	`uuid` text PRIMARY KEY NOT NULL,
	`parent_uuid` text,
	`session_id` text NOT NULL,
	`timestamp` integer NOT NULL,
	`message_type` text NOT NULL,
	`cwd` text NOT NULL,
	`user_type` text NOT NULL,
	`version` text NOT NULL,
	`isSidechain` integer NOT NULL,
	FOREIGN KEY (`parent_uuid`) REFERENCES `base_messages`(`uuid`) ON UPDATE no action ON DELETE no action
);
CREATE TABLE `user_messages` (
	`uuid` text PRIMARY KEY NOT NULL,
	`message` text NOT NULL,
	`tool_use_result` text,
	`timestamp` integer NOT NULL,
	FOREIGN KEY (`uuid`) REFERENCES `base_messages`(`uuid`) ON UPDATE no action ON DELETE no action
);
CREATE TABLE `conversation_summaries` (
	`leaf_uuid` text PRIMARY KEY NOT NULL,
	`summary` text NOT NULL,
	`updated_at` integer NOT NULL,
	FOREIGN KEY (`leaf_uuid`) REFERENCES `base_messages`(`uuid`) ON UPDATE no action ON DELETE no action
);

