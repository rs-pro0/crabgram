This project is in very early development, please don't use it.
# Installation
Because project is in early development it isn't just clone and run, and you need to manualy do some steps.\
So for now to make it work, you need to create empty "cache" directory.\
Then create database like this: ```sqlx db create --database-url sqlite:crabgram.db ```\
Then using sqlx cli apply photos migration like this ```sqlx migrate run --database-url sqlite:crabgram.db```\
Don't forget to set your .env up like this:
```
api_id = INSERT_API_ID_HERE
api_hash = INSERT_API_HASH_HERE
DATABASE_URL = sqlite:crabgram.db
```
Don't forget to clone https://github.com/lonami/grammers inside crabgram directory.
