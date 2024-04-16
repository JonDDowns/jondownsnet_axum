\set post `cat ./htmlpages/minified/RUserPythonGuide.html`
UPDATE posts SET post_body = :'post' WHERE post_id = 4;

