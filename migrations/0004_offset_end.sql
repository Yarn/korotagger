
alter table tags.stream_offsets
    add column "end" interval DEFAULT NULL
;
