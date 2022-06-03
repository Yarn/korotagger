
CREATE TYPE member.known_member_source AS ENUM ('gentei', 'badge');

ALTER TABLE member.known_members
ADD COLUMN "source" member.known_member_source;
UPDATE member.known_members SET source = 'gentei';
ALTER TABLE member.known_members ALTER COLUMN source SET NOT NULL;

CREATE TABLE "member"."blacklist" (
    "id" serial,
    "role" integer NOT NULL,
    "user_id" bigint NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("role") REFERENCES "member"."discord_roles"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "blacklist_role_user_id_idx" ON "member"."blacklist"("role","user_id");

CREATE TABLE "member"."whitelist" (
    "id" serial,
    "role" integer NOT NULL,
    "user_id" bigint NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("role") REFERENCES "member"."discord_roles"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "whitelist_role_user_id_idx" ON "member"."whitelist"("role","user_id");

CREATE TABLE "member"."required_roles" (
    "id" serial,
    "role" integer NOT NULL,
    "required_role" bigint NOT NULL,
    PRIMARY KEY ("id"),
    FOREIGN KEY ("role") REFERENCES "member"."discord_roles"("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "required_roles_role_required_role_idx" ON "member"."required_roles"("role","required_role");
