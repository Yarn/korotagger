
CREATE SCHEMA member;

CREATE TABLE "member"."yt_channels" (
    "id" serial,
    "yt_id" text NOT NULL,
    "readable" text DEFAULT null,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "channels_yt_id_idx" ON "member"."yt_channels"("yt_id");

CREATE TABLE "member"."gentei_slugs" (
    "id" serial,
    "channel" integer NOT NULL,
    "slug" text NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("channel") REFERENCES "member"."yt_channels"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "gentei_slugs_channel_idx" ON "member"."gentei_slugs"("channel");

CREATE TABLE "member"."known_members" (
    "id" serial,
    "discord_id" bigint NOT NULL,
    "channel" int NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("channel") REFERENCES "member"."yt_channels"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "known_members_discord_id_channel_idx" ON "member"."known_members"("discord_id","channel");
CREATE INDEX "known_members_channel_idx" ON "member"."known_members"("channel");

CREATE TABLE "member"."discord_roles" (
    "id" serial,
    "role_id" bigint NOT NULL,
    "guild_id" bigint NOT NULL,
    "yt_channel" integer NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("yt_channel") REFERENCES "member"."yt_channels"("id")
);
CREATE UNIQUE INDEX "discord_roles_role_id_idx" ON "member"."discord_roles"("role_id");
