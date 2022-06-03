CREATE SCHEMA tags;

CREATE TABLE "tags"."streams" (
    "id" serial,
    "name" text NOT NULL,
    "has_server" boolean NOT NULL,
    "server" bigint NOT NULL,
    "readable" text DEFAULT NULL,
    "start_time" timestamp NOT NULL,
    PRIMARY KEY ("id"),
    CHECK ((NOT has_server AND "server" = 0) OR (has_server))
);
CREATE UNIQUE INDEX "streams_name" ON "tags"."streams"("name", "has_server", "server");

CREATE TABLE "tags"."stream_offsets" (
    "id" serial,
    "order" integer NOT NULL,
    "stream" integer NOT NULL,
    "position" interval NOT NULL,
    "offset" interval NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("stream") REFERENCES "tags"."streams"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX "stream_offsets_stream" ON "tags"."stream_offsets"("stream");

CREATE TABLE "tags"."tags" (
    "id" serial,
    "stream" integer NOT NULL,
    "name" text NOT NULL,
    "time" timestamp NOT NULL,
    "server" bigint,
    "user" bigint,
    "message_id" bigint,
    "votes" integer NOT NULL DEFAULT '0',
    "deleted" boolean NOT NULL DEFAULT 'false',
    PRIMARY KEY ("id"),
    FOREIGN KEY ("stream") REFERENCES "tags"."streams"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX "tags_stream" ON "tags"."tags"("stream","deleted");

CREATE TABLE "tags"."tag_offsets" (
    "id" serial,
    "order" integer NOT NULL,
    "tag" integer,
    "offset" interval NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("tag") REFERENCES "tags"."tags"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE INDEX "tag_offsets_tag" ON "tags"."tag_offsets"("tag");

CREATE SCHEMA config;

CREATE TABLE "config"."server_admins" (
    "id" serial,
    "server" bigint NOT NULL,
    "group" bigint NOT NULL,
    "readable" text,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "server_admins_lookup" ON "config"."server_admins"("server","group");

CREATE TABLE "config"."admins" (
    "id" serial,
    "group" bigint NOT NULL,
    "readable" text,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "admins_lookup" ON "config"."admins"("group");

CREATE TABLE "config"."subscriptions" (
    "id" serial,
    "channel" bigint NOT NULL,
    "sub_id" text,
    "type" text,
    PRIMARY KEY ("id")
);
CREATE INDEX "subscriptions_channel" ON "config"."subscriptions"("channel");
CREATE UNIQUE INDEX "subscriptions_type_sub_id" ON "config"."subscriptions"("type","sub_id","channel");

CREATE TABLE "config"."selected_streams" (
    "id" serial,
    "channel" bigint NOT NULL,
    "stream" integer,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("stream") REFERENCES "tags"."streams"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "selected_streams_channel" ON "config"."selected_streams"("channel");

CREATE TABLE "config"."linked_channels" (
    "id" serial,
    "channel" bigint NOT NULL,
    "linked_to" bigint NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "linked_channels_channel" ON "config"."linked_channels"("channel");
CREATE INDEX "linked_channels_channel_to" ON "config"."linked_channels"("channel","linked_to");
